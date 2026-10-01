//! The many-body pass on the mesh: solve the field once for every node, then read each
//! node's share of it. The solve is [`Mesh::solve`]; this file is the read, as a range
//! kernel, and the merge.
//!
//! The read walks the nodes in the collide grid's order, as the deposit did, so
//! consecutive reads hit neighbouring cells.

use super::mesh::Mesh;
use crate::exec::{Runner, StepRange};
use crate::layout::force::barnes_hut::sim::{How, Sim};
use crate::layout::force::barnes_hut::{Split, step};
use std::ops::Range;

/// The per-node field read: `charge * alpha * E(x_i)`, node `order[k]` into slot `k`.
struct Interpolate<'a> {
    mesh: &'a Mesh,
    x: &'a [f64],
    y: &'a [f64],
    strength: f64,
}

impl StepRange for Interpolate<'_> {
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.mesh.grid.order.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        let order = &self.mesh.grid.order[range.start as usize..range.end as usize];
        for (slot, &i) in out.iter_mut().zip(order) {
            let (ex, ey) = self.mesh.field_at((self.x[i as usize], self.y[i as usize]));
            *slot = (ex * self.strength, ey * self.strength);
        }
    }
}

/// The many-body pass. Nothing moves when [`Mesh::solve`] finds no field.
pub(super) fn apply<R: Runner>(sim: &mut Sim, mesh: &mut Mesh, how: &mut How<'_, R>) {
    if !mesh.solve(sim) {
        return;
    }
    let read = Interpolate {
        mesh,
        x: &sim.x,
        y: &sim.y,
        strength: sim.params.charge * sim.alpha,
    };
    how.runner.run(&read, how.workers, how.deltas);
    let split = how.split.splits(Split::Charge);
    step::merge(
        (&mut sim.vx, &mut sim.vy),
        Some(&mesh.grid.order),
        how.deltas,
        split,
    );
}

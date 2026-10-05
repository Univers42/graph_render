//! The many-body pass on the mesh: solve the field once for every node, then read each
//! node's share of it. The solve is [`Mesh::solve`]; this file is the read, as a range
//! kernel, and the merge.
//!
//! The read walks the nodes in the collide grid's order, as the deposit did, so
//! consecutive reads hit neighbouring cells.

use super::mesh::Mesh;
use super::motion::{self, Gathered};
use crate::exec::{Runner, StepRange};
use crate::layout::force::barnes_hut::Split;
use crate::layout::force::barnes_hut::sim::{How, Sim};
use std::ops::Range;

/// The per-node field read: `charge * alpha * E(x_i)`, node `order[k]` into slot `k`, from
/// the stencil the deposit stored for slot `k` rather than the node's position.
struct Interpolate<'a> {
    mesh: &'a Mesh,
    strength: f64,
}

impl StepRange for Interpolate<'_> {
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.mesh.grid.order.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        for (slot, k) in out.iter_mut().zip(range) {
            let (ex, ey) = self.mesh.field_of(k as usize);
            *slot = (ex * self.strength, ey * self.strength);
        }
    }
}

/// The many-body pass. Nothing moves when [`Mesh::solve`] finds no field.
pub(super) fn apply<R: Runner>(sim: &mut Sim, mesh: &mut Mesh, how: &mut How<'_, R>) {
    if !read(&*sim, mesh, how) {
        return;
    }
    merge(sim, mesh, how);
}

/// [`Mesh::solve`] and the field read into `how.deltas` in slot order, and nothing else:
/// the tick fuses this merge with the link pass's, so the read has to stand on its own.
/// `false` when there is no field, and then nothing was written.
pub(super) fn read<R: Runner>(sim: &Sim, mesh: &mut Mesh, how: &mut How<'_, R>) -> bool {
    if !mesh.solve(sim, how.runner, how.workers) {
        return false;
    }
    let field = Interpolate {
        mesh,
        strength: sim.params.charge * sim.alpha,
    };
    how.runner.run(&field, how.workers, how.deltas);
    true
}

/// The charge deltas' merge: node `i` gathers its own slot, under [`Split::Charge`].
pub(super) fn merge<R: Runner>(sim: &mut Sim, mesh: &Mesh, how: &How<'_, R>) {
    let gathered = Gathered {
        deltas: &how.deltas[..],
        slot: Some(&mesh.grid.slot),
        split: how.split.splits(Split::Charge),
    };
    motion::merge(sim, gathered, (how.runner, how.workers));
}

//! The many-body probe: what this session's engine's charge pass alone would do to the
//! velocities, with no tick, no decay and no other force around it.
//!
//! # Why this is not a second implementation
//!
//! It builds no force law of its own. It takes a *copy* of the session's state, stops the
//! velocities, puts `alpha` at 1 and calls the engine's own many-body pass — Barnes-Hut's
//! charge walk, or the mesh's field read, whichever this session ticks with. So the number
//! it hands back is the solver's, and comparing two of them is comparing two solvers.
//!
//! # Why the copy, and why the velocities at rest
//!
//! The pass *accumulates into* `vx`/`vy` and adds the velocity decay's worth of nothing
//! else, so a pass run against a live session's own columns would hand back the session's
//! motion plus the force. [`Sim::from_parts`] starts every velocity column at zero and
//! that is what the pass reads, which is the whole point: at rest, `vx` after the pass is
//! the force, to the last bit.
//!
//! # `theta`
//!
//! Barnes-Hut reads it from the [`Sim`]'s parameters (`charge.rs`'s `prepare`), so the
//! probe sets it there on its own copy and nothing else in the crate learns a new knob:
//! [`ForceParams::default`](crate::layout::force::ForceParams) is untouched, and the hash
//! gate is what says so. The mesh has no `theta` and the argument is ignored there.
//!
//! Caveat: the pass reads `sim.seed` and `sim.tick_no` for its coincidence jiggle, and a
//! probe copy is at tick 0 whatever the session's own tick is. Two coincident points are
//! the only input that notices, and `charge_deltas` is not the instrument for coincident
//! points — the exact all-pairs reference skips them and counts them instead.

use super::ForceSession;
use crate::exec::Serial;
use crate::layout::force::Split;
use crate::layout::force::barnes_hut::charge_pass as barnes_hut_pass;
use crate::layout::force::barnes_hut::sim::{How, Sim};
use crate::layout::force::particle_mesh::{self, Mesh};

/// The runner the probe divides nothing by: a probe's answer is a number a report prints,
/// not a tick, so there is no tier to choose and one worker is the only honest schedule.
const WORKERS: u32 = 1;

impl ForceSession {
    /// This session's engine's many-body force at this session's positions, at `alpha` 1
    /// and with every velocity at rest, as `(dvx, dvy)` per row in row order.
    ///
    /// An instrument for `graph-cli mb-fidelity`: it measures how far a many-body solver
    /// is from the exact all-pairs sum, and nothing in the product calls it. `theta` is
    /// Barnes-Hut's opening angle (`bh:<theta>`); the particle mesh ignores it.
    ///
    /// Does not move the session: positions, velocities, parameters, pins and the tick
    /// number are exactly as they were on entry and on return.
    ///
    /// Caveat: this is the many-body force and nothing else. Link, center, collide,
    /// gravity, the cooling schedule and the integration are all absent by construction,
    /// so a number from here says how close one force is, not how close a layout run is.
    pub fn charge_deltas(&self, theta: f64) -> (Vec<f64>, Vec<f64>) {
        let runner = Serial;
        let rows = self.sim.rows();
        let mut sim = self.charge_probe(theta);
        let mut deltas = Vec::with_capacity(rows as usize);
        if self.mesh.is_some() {
            let mut mesh = Mesh::new(rows);
            let mut how = How {
                runner: &runner,
                workers: WORKERS,
                deltas: &mut deltas,
                split: Split::None,
            };
            particle_mesh::charge_pass(&mut sim, &mut mesh, &mut how);
        } else {
            barnes_hut_pass(&mut sim, &runner, WORKERS, &mut deltas);
        }
        (sim.vx, sim.vy)
    }

    /// The session's own state as a fresh [`Sim`] at the given `theta`, every velocity at
    /// rest and `alpha` at 1.
    ///
    /// Positions, the simple graph and the seed are the session's; everything else the
    /// many-body pass reads is its two `distance` bounds and `charge`, which travel with
    /// the parameters. The link geometry [`Sim::from_parts`] recomputes on the way is
    /// unused by this pass — it is paid once per probe, which is a report's cost and not a
    /// tick's.
    fn charge_probe(&self, theta: f64) -> Sim {
        let mut params = self.sim.params;
        params.theta = theta;
        let graph = self.sim.graph.clone();
        let positions = (self.sim.x.clone(), self.sim.y.clone());
        let mut sim = Sim::from_parts(graph, params, self.sim.seed, positions);
        sim.alpha = 1.0;
        sim
    }
}

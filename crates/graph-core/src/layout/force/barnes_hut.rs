//! Barnes-Hut force layout: `manyBody` (charge) via the quadtree, `link` and `collide`
//! ported to Jacobi gathers with canonical pair orientation (devil C7), `center` as a
//! plain mean-and-translate. `prompt.md` §3.1, Phase 6 branch p6f, ported from
//! `/home/user/refs/npm/d3-force-3.0.0`. Split across `barnes_hut/{seed,sim,link,charge,
//! collide}.rs` for the house line cap — the same split `quadtree.rs`/`quadtree/tests.rs`
//! already uses, reported as a deviation from the branch's file list, which named
//! `barnes_hut.rs` alone.
//!
//! Deviates from d3-force in five documented ways (also recorded in
//! `docs/measurements/phase06-stress.md`):
//! 1. link and collide are Jacobi (not Gauss-Seidel) gathers (devil C7).
//! 2. `jiggle` is the counter-based hash in [`crate::rng`], not a seeded LCG (devil C8).
//! 3. link's degree bias uses the pair's lower/higher node index in place of the raw
//!    graph's own (now-discarded) source/target direction (`barnes_hut/link.rs`).
//! 4. seed positions centre on the origin, not a viewport (`barnes_hut/seed.rs`).
//! 5. there is no cluster force (Phase 10's node groups do not exist yet).

mod charge;
mod collide;
mod link;
mod seed;
mod sim;
mod step;

#[cfg(test)]
mod tests;

use super::params::{ForceParams, TICKS};
use crate::exec::Serial;
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use sim::{How, Sim};

/// Barnes-Hut approximated force layout (`prompt.md` §3.1).
///
/// Ponytail: force layouts are chaotic — the same graph with one node added or removed
/// is a different picture, not a perturbed one; there is no failing input narrower than
/// "any topology change". Direction: cosmetic-but-surprising, not silently wrong (the
/// stress metric, not visual stability, is what this layout is graded on). Escape hatch:
/// a fixed seed and this stage's own determinism — the same topology, run twice, settles
/// to the same geometry every time (`barnes_hut/tests.rs`'s
/// `the_same_topology_settles_to_the_same_geometry_run_to_run`).
pub struct BarnesHut;

impl Stage for BarnesHut {
    type Params = ForceParams;
    const ID: &'static str = "layout.force.barnes_hut";

    /// The serial tier, which is the stage: [`run_with`](Self::run_with) over
    /// [`Serial`] with one worker, and the arm every other runner must hash-equal.
    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        Self::run_with(topology, params, &Serial, 1)
    }
}

impl BarnesHut {
    /// The same layout, with the many-body pass handed to `runner` over `workers` workers.
    ///
    /// The point of the signature is what it does **not** change: the tick order, the
    /// `alpha` decay, the tree build, the link and collide passes and the integration are
    /// all the same code the serial stage runs, and only the division of the many-body
    /// queries differs. So `run_with(..., &Serial, 1)` is [`Stage::run`] by construction,
    /// and any other runner is a *schedule* of the same computation — which is the claim
    /// the N-way hash gate checks and `barnes_hut/tests.rs` pins at the unit level.
    ///
    /// `workers` below 2 is the serial path (see [`crate::exec::Runner`]), so a host
    /// reporting no threads gets the same bytes rather than a fast failure.
    pub fn run_with(
        topology: &Topology,
        params: &ForceParams,
        runner: &impl crate::exec::Runner,
        workers: u32,
    ) -> Result<Geometry, StageError> {
        Self::run_under(topology, params, runner, workers, false)
    }

    /// [`run_with`](Self::run_with) with the negative control reachable, so the host can
    /// run a *deliberately wrong* tier and the gate must go red.
    ///
    /// Separate from [`run_with`](Self::run_with) rather than a defaulted argument on it
    /// because the default would be one boolean away from a stage that silently mutated
    /// itself — and a control that a caller can forget to pass is not a control, it is a
    /// fourth path nobody exercises.
    pub fn run_under(
        topology: &Topology,
        params: &ForceParams,
        runner: &impl crate::exec::Runner,
        workers: u32,
        split_sum: bool,
    ) -> Result<Geometry, StageError> {
        let mut sim = Sim::new(topology, *params, 0);
        let mut deltas: Vec<(f64, f64)> = Vec::new();
        for _ in 0..TICKS {
            let mut how = How {
                runner,
                workers,
                deltas: &mut deltas,
                split_sum,
            };
            sim.tick(&mut how);
        }
        let (x, y) = sim.positions();
        if x.iter().chain(y).any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite { column: "node.x" });
        }
        Ok(Geometry {
            nodes: NodeGeometry::Point {
                x: x.iter().map(|&v| v as f32).collect(),
                y: y.iter().map(|&v| v as f32).collect(),
            },
            edges: EdgeGeometry::Line,
            notes: Vec::new(),
        })
    }
}

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
//!
//! The stage itself is now a thin shell over `super::session`: it builds a session at the
//! frozen parameters, with no pins and `alpha_target` 0, and steps it `TICKS` times — the
//! same forces, the same quadtree, the same tick it always ran, expressed as the degenerate
//! case of the live one (`docs/decisions/live-force-session.md`).

mod charge;
mod collide;
mod link;
mod seed;
pub(in crate::layout::force) mod sim;

#[cfg(test)]
mod tests;

use super::params::{ForceParams, TICKS};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::session::ForceSession;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// Barnes-Hut approximated force layout (`prompt.md` §3.1).
///
/// **The frozen layout is a session, not a layout.** What this stage runs is
/// `ForceSession::from_frozen(topology, params)`, no pins, `alpha_target` 0,
/// stepped [`TICKS`] times — the same forces, the same quadtree, the same tick the live
/// session owns, and the same bytes it always produced
/// (`docs/decisions/live-force-session.md`; pinned over 65 seeds by
/// `session/tests/m1a.rs`).
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

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let mut session = ForceSession::from_frozen(topology, params)?;
        session.step(TICKS);
        let (x, y) = (session.xs(), session.ys());
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

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

#[cfg(test)]
mod tests;

use super::params::{ForceParams, TICKS};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use sim::Sim;

/// Barnes-Hut approximated force layout (`prompt.md` §3.1).
pub struct BarnesHut;

impl Stage for BarnesHut {
    type Params = ForceParams;
    const ID: &'static str = "layout.force.barnes_hut";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let mut sim = Sim::new(topology, *params, 0);
        for _ in 0..TICKS {
            sim.tick();
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

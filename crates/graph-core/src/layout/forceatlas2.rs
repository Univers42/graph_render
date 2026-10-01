//! ForceAtlas2: a direct port of networkx 3.6's `forceatlas2_layout`
//! (`networkx/drawing/layout.py` in the pinned networkx-3.6 source, `fetch-refs.sh`), scoped to its default
//! configuration (see `forceatlas2/state.rs`'s doc comment for the exact list). Split
//! across `forceatlas2/{state,tests}.rs` for the house line cap — the same split
//! `quadtree.rs`/`quadtree/tests.rs` and `barnes_hut.rs`/`barnes_hut/*.rs` already use,
//! reported as a deviation from the branch's file list, which named `forceatlas2.rs`
//! alone. `prompt.md` §3.1, Phase 6 branch p6f.

mod state;

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use state::Fa2State;
pub use state::{Fa2Params, initial_positions};

/// ForceAtlas2 layout (`prompt.md` §3.1): a dense, O(n²)-per-iteration alternative to
/// [`super::force::BarnesHut`] for the graph sizes it stays practical at — see
/// `docs/measurements/phase06-force.md` for the measured ceiling.
///
/// Ponytail: force layouts are chaotic — the same graph with one node added or removed
/// is a different picture, not a perturbed one (`phase-06-iterative-spectral-mds.md`'s
/// Ponytail requirements; see [`super::force::BarnesHut`]'s own marker for the full
/// direction/escape-hatch statement, identical here).
pub struct ForceAtlas2;

impl Stage for ForceAtlas2 {
    type Params = Fa2Params;
    const ID: &'static str = "layout.forceatlas2";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let mut state = Fa2State::new(topology, *params);
        state.run();
        let (x, y) = state.positions();
        if x.iter().chain(y).any(|v| !v.is_finite()) {
            return Err(StageError::NonFinite { column: "node.x" });
        }
        Ok(Geometry::planar(
            NodeGeometry::Point {
                x: x.iter().map(|&v| v as f32).collect(),
                y: y.iter().map(|&v| v as f32).collect(),
            },
            EdgeGeometry::Line,
            Vec::new(),
        ))
    }
}

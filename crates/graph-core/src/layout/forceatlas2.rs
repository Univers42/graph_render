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
pub use state::{Fa2Params, initial_positions};
use state::{Fa2State, MAX_DIM};

/// SciGraphs' `FORCEATLAS2` runs at `dim = 3` by default (`forceatlas.py:153`); this is
/// the same kernel at 3. `layout.forceatlas2` is this arm at `dim` 2.
pub const ID_3D: &str = "layout.forceatlas2.3d";

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
        run_at_dim(topology, params, params.dim)
    }
}

/// ForceAtlas2 with its repulsion summed over a quadtree (`forceatlas2/state/barnes_hut.rs`):
/// O(n log n) per iteration where [`ForceAtlas2`] is O(n²), every other force and the
/// adaptive speed unchanged. [`ForceAtlas2`] is its oracle.
///
/// Ponytail: chaotic as [`ForceAtlas2`] is, and the far-field approximation's own limit is
/// the `Caveat:` in `barnes_hut.rs`.
pub struct ForceAtlas2BarnesHut;

impl Stage for ForceAtlas2BarnesHut {
    type Params = Fa2Params;
    const ID: &'static str = "layout.forceatlas2.barnes_hut";

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        // `dim` is pinned at 2 here rather than read from `params`: the tree sums two axes,
        // so a `params.dim` of 3 would leave the third unrepelled instead of refusing. The
        // arm has no 3D id and no 3D differential, so nothing here can honour a third axis.
        settle(Fa2State::new(topology, *params, 2).with_tree())
    }
}

/// The 3D arm: [`Stage::run`] with `dim` forced to 3. See
/// [`super::force::fruchterman_reingold::run_3d`] for why it is a function and not a
/// second `Stage` impl.
pub fn run_3d(topology: &Topology, params: &Fa2Params) -> Result<Geometry, StageError> {
    run_at_dim(topology, params, MAX_DIM)
}

fn run_at_dim(topology: &Topology, params: &Fa2Params, dim: usize) -> Result<Geometry, StageError> {
    if !(2..=MAX_DIM).contains(&dim) {
        return Err(StageError::Param {
            name: "dim",
            rule: "2 or 3 coordinates per node",
        });
    }
    settle(Fa2State::new(topology, *params, dim))
}

/// Runs `state` to the end and narrows its positions to the wire, or refuses a non-finite
/// one. The only place a dimension becomes a [`Geometry`] in this kernel.
fn settle(mut state: Fa2State) -> Result<Geometry, StageError> {
    let dim = state.dim();
    state.run();
    let pos = state.positions();
    if pos.iter().flatten().take(dim).any(|v| !v.is_finite()) {
        return Err(StageError::NonFinite { column: "node.x" });
    }
    let column = |a: usize| pos.iter().map(|p| p[a] as f32).collect();
    Ok(Geometry::points(dim, column(0), column(1), column(2)))
}

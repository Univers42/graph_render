//! The six overrides of [`super`], one function each, plus the spring arm that writes its
//! defaults out: where the registered default is not SciGraphs' parameter or units.

use super::super::fixtures::Fixture;
use super::super::{ITERATIONS, LAYOUT_SEED, SCALE};
use super::finish;
use graph_contract::binary::SnapshotParts;
use graph_core::Stage;
use graph_core::exec::Serial;
use graph_core::layout::circle_packing::{self, CirclePackingParams};
use graph_core::layout::force::spring::SpringParams;
use graph_core::layout::forceatlas2::{Fa2Params, ForceAtlas2};
use graph_core::layout::graphviz::sfdp;
use graph_core::layout::grid::Grid;
use graph_core::layout::random;

/// `CirclePackingParams` at SciGraphs' two numbers, which its registered default is not:
/// `iterations` is 500 there and `apply_graph_layout` passes 50 (`circle_packing.py:281`).
pub(super) fn packing(fixture: &Fixture) -> Result<SnapshotParts, String> {
    let params = CirclePackingParams {
        iterations: ITERATIONS,
        scale: SCALE as f32,
    };
    finish(fixture, circle_packing::ID, |t| {
        circle_packing::run_with(t, &params)
    })
}

/// The layered DAG drawing on SciGraphs' axes: the registered `layout.dag.sugiyama` plus
/// its per-axis normalisation (`hierarchical.py:679-685`), at the dispatcher's `scale`.
///
/// **A fourth override, and the only one that changes units rather than numbers.** The
/// registered layout draws X in the priority method's own units and Y as
/// `layer * LAYER_SPACING`, neither centred; the reference maps both onto `[-scale, scale]`.
/// The normalisation cannot be a post pass here, because its `lo`/`hi` are the extremes
/// over the whole ordering graph, dummy vertices included, and `Geometry` carries no dummy
/// coordinates — so it is a second entry point in graph-core (`sugiyama::run_scaled`) beside
/// the stages that produce its inputs, not a transform in this file. `layout.dag.sugiyama`
/// itself is untouched: the registry default still draws in its own units, because that is
/// what the dagre differential measures.
pub(super) fn sugiyama_scaled(fixture: &Fixture) -> Result<SnapshotParts, String> {
    finish(fixture, "layout.dag.sugiyama", |t| {
        graph_core::layout::sugiyama::run_scaled(t, SCALE as f32)
    })
}

/// `Fa2Params` at the dispatcher's `iterations` and the layout seed, which its registered
/// default is not: `max_iter` is 100 there and `seed` is 0 (`forceatlas.py:150`,
/// `dispatcher.py:62`).
pub(super) fn fa2(fixture: &Fixture) -> Result<SnapshotParts, String> {
    let params = Fa2Params {
        max_iter: ITERATIONS,
        seed: LAYOUT_SEED,
        ..Fa2Params::default()
    };
    finish(fixture, ForceAtlas2::ID, |t| ForceAtlas2::run(t, &params))
}

/// sfdp at the layout seed. `sfdp::run` hard-codes `DEFAULT_SEED = 1`; the engine is handed
/// `start = get_layout_seed()` (`yifan_hu.py:229`), so the seeded entry point is the one
/// that answers the question.
pub(super) fn sfdp_seeded(fixture: &Fixture) -> Result<SnapshotParts, String> {
    finish(fixture, sfdp::ID, |t| sfdp::run_seeded(t, LAYOUT_SEED))
}

/// The spring kernel, both dimensions, at SciGraphs' `iterations` and `scale` — which are
/// already `SpringParams`' own defaults, so this arm writes them out anyway so a reader can
/// see the two arms were handed the same numbers rather than the same defaults.
pub(super) fn spring<S: Stage<Params = SpringParams>>(
    fixture: &Fixture,
) -> Result<SnapshotParts, String> {
    let params = SpringParams {
        iterations: ITERATIONS,
        scale: SCALE,
        ..SpringParams::default()
    };
    finish(fixture, S::ID, |t| S::run(t, &params))
}

/// `layout.random` at the layout seed, which its registered default is not: `random::run`
/// is networkx's planar unit-square scatter off the crate's own `Mulberry32` at a house
/// seed, and SciGraphs' `_random_layout` (`basic.py:5-9`) is
/// `np.random.RandomState(get_layout_seed()).rand(n, 3) * scale` — three axes, scaled, off
/// MT19937. `run_seeded` is the same shape as [`sfdp_seeded`], and the registered default is
/// left alone so its hash-gate record stands.
pub(super) fn random_seeded(fixture: &Fixture) -> Result<SnapshotParts, String> {
    finish(fixture, random::ID, |t| random::run_seeded(t, LAYOUT_SEED))
}

/// The grid at SciGraphs' `scale`, which its registered default is not: `_grid_layout`
/// (`basic.py:11-20`) starts the first cell **at the origin** and sets the pitch to
/// `scale / grid_size`, where the registered stage centres the lattice at
/// `GridParams::spacing = 1.0` and lets nothing rescale it. `Grid::run_scaled` is that
/// placement, and it is `f64` inside because a `f32` pitch is a whole ULP off
/// (`layout/grid/scaled.rs`).
pub(super) fn grid(fixture: &Fixture) -> Result<SnapshotParts, String> {
    finish(fixture, Grid::ID, |t| {
        Grid::run_scaled(t, SCALE, &Serial, 1)
    })
}

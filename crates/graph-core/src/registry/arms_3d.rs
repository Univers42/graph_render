//! The 3D arms: their ledger metadata **and** the run shims `LAYOUTS` points at.
//!
//! Kept apart from `registry.rs` and `registry/igraph.rs` for the house line cap, and it is
//! the one module that knows about all five arms — a shim next to its own metadata row, so a
//! row cannot claim an oracle its shim does not run.
//!
//! Rules and reference pin: `docs/decisions/layouts-igraph.md`. The SciGraphs dispatcher
//! arms these are ported from are `_forceatlas2_layout` (`forceatlas.py:150`, `dim=3` at
//! `:153`), `_igraph_fruchterman_reingold` (`igraph_layouts.py:53`, `'dim': 3` at `:74`),
//! `_igraph_kamada_kawai` (`:85`, `dim` at `:99`), `_igraph_drl` (`:281`, `dim` at `:342`)
//! and `_yifan_hu_layout`'s `'2Z'` mode (`yifan_hu.py:344`, `:327-333`).

use super::force::{FA2_CEILING, FORCE_CEILING};
use super::{Capability, LayoutParams, Metadata};
use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::drl::DRL_CEILING;
use crate::layout::force::fruchterman_reingold::FR_CEILING;
use crate::layout::force::kamada_kawai::KK_CEILING;
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// `layout.force.fruchterman_reingold.3d`: [`super::igraph::FRUCHTERMAN_REINGOLD`] at
/// `dim = 3`.
///
/// The measurement fields are the 2D row's because it is the same algorithm at another
/// dimension — `niter` 500, the same dense O(n²) pair loop, so the node count at which it
/// stops being usable does not move. What differs is stated, not inherited silently: the
/// oracle names the 3D call and its own differential, and the start is a cube where
/// igraph draws inside a ball.
pub(super) const FRUCHTERMAN_REINGOLD_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_fruchterman_reingold at dim=3, niter=500, in \
    the ge-python-oracle image, started from our own 3D start; harness/oracle-igraph.py \
    compares layout stress (ours/igraph, ceiling in graph-cli oracle_python/igraph.rs), not \
    coordinates, and against its own 3D fixture set and its own measured ceiling",
    complexity: "O(niter * (n^2 + m)) with niter = 500, dense repulsion over three axes; \
    O(n + m) memory",
    scale_ceiling: FR_CEILING,
    degradation: "past the ceiling there is no refusal: the dense loop still returns finite \
    geometry, only slower (quadratically), so the caller applies its own timeout; igraph's \
    grid variant for n > 1000 is 2D only, so the 3D arm has no grid variant to differ from; \
    a non-finite position refuses with StageError::NonFinite",
    ponytail: "Ponytail: chaotic like every force layout - one added node is a different \
    picture, not a perturbed one; the direction is cosmetic, never silently wrong. Ponytail \
    (start): the 3D start is this port's cube of side sqrt(n) and igraph's is a ball, so the \
    two pictures differ from the first iteration - cosmetic, and the differential compares \
    stress rather than positions. Ponytail (scale_ceiling): inherited from the 2D arm and \
    NOT re-measured in 3D; the pair loop is the same dense O(n^2) with one more term per \
    pair, so the ceiling moves by a factor near 1 that a measurement would not resolve",
};

/// `layout.force.kamada_kawai.3d`: [`super::igraph::KAMADA_KAWAI`] at `dim = 3`.
///
/// The one field that genuinely moves is complexity: the Newton step solves a 3x3 block
/// where the 2D arm solves 2x2, so it is cubic in the (constant) dimension per move and
/// still O(n²) overall — the dimension is a constant factor, not a change of order, and
/// that is why the ceiling is the 2D one rather than a smaller number.
pub(super) const KAMADA_KAWAI_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_kamada_kawai at dim=3 (default maxiter 50 n) \
    in the ge-python-oracle image, started from our own sphere start; \
    harness/oracle-igraph.py compares layout stress (ours/igraph, ceiling in graph-cli \
    oracle_python/igraph.rs), not coordinates, against its own 3D fixture set",
    complexity: "O(n^2) set-up (all-pairs BFS, one n x n matrix) + O(50 n * n) moves, each \
    solving a 3x3 Newton block by adjugate instead of 2x2, so O(n^2) with a larger constant",
    scale_ceiling: KK_CEILING,
    degradation: "past the ceiling there is no refusal: memory grows as 8 n^2 bytes and time \
    as 50 n^2, so a caller applies its own timeout; a singular Newton block gives a zero \
    step rather than a division by a near-zero determinant, so a vertex can stop moving; an \
    edgeless graph with n >= 2 takes every distance as 1 instead of dividing by zero; a \
    non-finite position refuses with StageError::NonFinite",
    ponytail: "Ponytail: Newton descent finds a local minimum of the spring energy, so a \
    folded start can settle folded - cosmetic, never invalid. Ponytail (start): the 3D start \
    is a Fibonacci sphere of the spec's own start radius, which igraph does not use, so the \
    pictures differ from the first move; the sphere is required rather than decorative, since \
    a 3D start drawn in a plane leaves the first Newton step with a singular zz block. \
    Ponytail (scale_ceiling): inherited from the 2D arm and NOT re-measured in 3D",
};

/// `layout.force.drl.3d`: [`super::igraph::DRL`] at `dim = 3`. SciGraphs' `IGRAPH_DRL`
/// (`igraph_layouts.py:281`) is the 3D one and `IGRAPH_DRL_2D` (`:307`) the 2D, which
/// stays on `layout.force.drl`.
///
/// The density field is the one place the shape really changes: the coarse 1000 x 1000
/// separable grid has no third axis, so the 3D arm bins node positions into cells and sums
/// `1/d^2` over the real 3D distance from the start, where the 2D arm switches to that
/// same binning only in its last stage. That is a stated design choice, and it is the
/// reason the 3D row's complexity line differs.
pub(super) const DRL_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "python-igraph 0.11.9 Graph.layout_drl at dim=3 (default preset) in the \
    ge-python-oracle image, started from our own 3D start; harness/oracle-igraph.py \
    compares layout stress (ours/igraph, ceiling in graph-cli oracle_python/igraph.rs), not \
    coordinates: igraph works in single precision and the liquid stage amplifies any \
    difference",
    complexity: "O(S * n * deg) with S about 550 sweeps; the density is a per-cell bin list \
    over the 3D view from the first sweep, so no 8 MB grid is allocated at all",
    scale_ceiling: DRL_CEILING,
    degradation: "past the ceiling there is no refusal: sweeps stay linear in n, only slower; \
    a node outside the 3D view is held by the border wall instead of raising an error, so a \
    layout that spreads wider than the view settles on the analytic pull alone; a \
    non-finite position refuses with StageError::NonFinite",
    ponytail: "Ponytail: sequential index-ordered sweeps make the picture depend on node \
    numbering, and edge cutting is permanent and one-directional - cosmetic, never invalid. \
    Ponytail (density): the coarse separable grid is 2D, so the 3D arm uses the per-cell \
    1/d^2 bins from the first sweep rather than a 3D grid that does not exist - a stated \
    choice, and the escape hatch is Density::new's dim. Ponytail (scale_ceiling): \
    inherited from the 2D arm and NOT re-measured in 3D",
};

/// `layout.force.yifan_hu.2z`: [`super::force::YIFAN_HU`] plus a derived third axis.
///
/// The only row here whose 3D arm is **not** the force run in three dimensions, and the
/// metadata says so: SciGraphs' `'2Z'` mode (`yifan_hu.py:346`) is `'2'` in the plane with
/// a `z` read from the graph by `_generate_z_component`, so the simulation stays 2D and
/// the third axis never steers it. The complexity line is therefore the 2D row's own, plus
/// one eigensolve per component for the z.
pub(super) const YIFAN_HU_2Z: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "no coordinate oracle exists and none is claimed: this is not sfdp and no sfdp \
    output was compared, exactly as for the 2D arm; the stress metric (harness/stress-d3.mjs, \
    record `stress`) is the closest thing, and the z column is checked by this crate's own \
    tests rather than against a reference",
    complexity: "O(n log n) x (112 + 48 x levels) for the 2D multilevel solve, unchanged from \
    the 2D arm, plus one 2D spectral eigensolve per connected component for the derived z \
    (dense below 256 nodes per component, LOBPCG above)",
    scale_ceiling: FORCE_CEILING,
    degradation: "past the ceiling there is no refusal: the levels stay finite, only slower, \
    so the caller applies its own timeout; a component whose spectral solve fails the \
    residual/orthonormality gate contributes zeros to z, which is the reference's own \
    DEGREE-fallback path; a layout whose extent is zero yields a zero z column rather than \
    a division by zero; a non-finite position refuses with StageError::NonFinite",
    ponytail: "Ponytail: the z is read from the graph and never steers the simulation, so \
    the third axis is decoration a user cannot interact with - cosmetic, never silently \
    wrong, and the escape hatch is the 2D id. Ponytail (scale_ceiling): inherited from the \
    2D arm, which is honest here precisely because the force run IS the 2D one. Ponytail (z \
    scale): the reference multiplies by its own scale argument, this multiplies by the \
    layout's own extent, so the ratio 0.3 of the drawing is kept but the absolute size is \
    ours",
};

/// `layout.force.yifan_hu.3d`: the 3D yifan_hu, as SciGraphs' `'3'` mode asks for
/// (`yifan_hu.py:344`).
///
/// The one row here whose force run **is** three-dimensional: `z` is a coordinate in the
/// same difference vectors as `x` and `y`, pushed by many-body, link and collide and pushing
/// back. That is what separates it from [`YIFAN_HU_2Z`] above, whose `z` is derived from the
/// graph after the run and never steers it. The per-level solver is a sibling 3D tick over
/// an octree, not the 2D one — see `layout/force/yifan_hu/arm3d.rs`.
pub(super) const YIFAN_HU_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "no coordinate oracle exists and none is claimed, for the same reason as the 2D \
    arm: this is not sfdp and no sfdp output was compared, in any dimension; the stress \
    metric (harness/stress-d3.mjs, record `stress`) is the closest thing and it is a 2-axis \
    metric, so this row's own evidence is graph-core's tests — three finite equal-length \
    columns, a z that spreads, a z that is not the 2D arm's x, and byte-identical output on \
    a repeat run — rather than a differential",
    complexity: "O(n log n) x (112 + 48 x levels) for the multilevel solve, the 2D arm's \
    own cost over three axes: the many-body walk is Barnes-Hut over an octree and collide is \
    O(n log n) over a second octree, both stackless preorder walks; O(n + m) memory plus the \
    two octree arenas",
    scale_ceiling: FORCE_CEILING,
    degradation: "past the ceiling there is no refusal: the levels stay finite, only \
    slower, so the caller applies its own timeout; a non-finite position refuses with \
    StageError::NonFinite and the refusal covers z as well as x and y, because a 3D arm that \
    validated only its in-plane columns would ship a NaN z as a finite-looking picture; two \
    nodes closer together than one f64 step of the cube holding them are chained onto one \
    leaf and charged at the chain head's jiggled separation rather than at their own; a \
    single node settles to one point per axis rather than dividing by zero",
    ponytail: "Ponytail: chaotic like every force layout - one added node is a different \
    picture, not a perturbed one; the direction is cosmetic, never silently wrong. Ponytail \
    (start): the 3D start is this port's own Fibonacci sphere at the 2D spiral's radius, \
    which no reference uses, so the pictures differ from the first iteration - cosmetic, and \
    the escape hatch is barnes_hut::seed::sphere_point. Ponytail (tier): the 3D tick has no \
    threaded tier, so unlike the 2D arm this row has no worker-count dimension to \
    hash-equal across - one schedule, one set of bytes. Ponytail (scale_ceiling): inherited \
    from the 2D arm and NOT re-measured in 3D; the octree makes many-body O(n log n) in \
    three dimensions exactly as in two, and the constant grows by a factor near 1 that a \
    measurement would not resolve",
};

/// `layout.forceatlas2.3d`: [`super::force::FA2`] at `dim = 3`, which is what SciGraphs'
/// `FORCEATLAS2` asks for by default (`forceatlas.py:153`).
///
/// FA2 is a *dense* all-pairs layout, so its 3D arm needs no tree at all: the third
/// coordinate is one more term in the same difference vector. Its oracle is networkx 3.6's
/// `forceatlas2_layout` at `dim = 3`, compared on coordinates rather than stress — the
/// only force row in this file that is, because FA2's chaotic amplification is bounded by
/// the same gated iteration budget the 2D row uses and that argument is dimension-free.
pub(super) const FA2_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx 3.6 forceatlas2_layout at dim=3 in the ge-python-oracle image, started \
    from the port's own 3D initial positions so both arms run the same iterations from the \
    same state; harness/oracle-fa2.py compares coordinates (worst max|ours-theirs| over three \
    axes, divided by the reference extent) under its own 3D ceiling",
    complexity: "O(n^2) per iteration, dense, three axes; O(n + m) memory",
    scale_ceiling: FA2_CEILING,
    degradation: "past the ceiling there is no refusal: the dense loop still returns finite \
    geometry, only slower, so the caller applies its own timeout; a non-finite position \
    refuses with StageError::NonFinite",
    ponytail: "Ponytail: chaotic like every force layout - the gated iteration budget is the \
    2D arm's, and the 3D arm is measured at that budget against its own ceiling rather than \
    the 2D one. Ponytail (scale_ceiling): inherited from the 2D arm and NOT re-measured in \
    3D; the loop is the same dense O(n^2) with one more term per pair",
};

/// The run shims [`LAYOUTS`](super::LAYOUTS) points at, one per 3D arm.
///
/// **Five named functions rather than one generic helper**, because the registry's `run` slot
/// is a `fn(&Topology) -> Result<Geometry, StageError>` pointer, so each arm needs a
/// non-generic entry, and a name per arm is what keeps the table readable. Each delegates to
/// its kernel's own `run_3d` / `run_2z`, so the dimension stays a *parameter* of one kernel
/// and none of these is a second implementation.
pub(super) fn run_fr_3d(topology: &Topology) -> Result<Geometry, StageError> {
    crate::layout::force::fruchterman_reingold::run_3d(
        topology,
        &crate::layout::force::fruchterman_reingold::FrParams::default(),
    )
}

pub(super) fn run_kk_3d(topology: &Topology) -> Result<Geometry, StageError> {
    crate::layout::force::kamada_kawai::run_3d(
        topology,
        &crate::layout::force::kamada_kawai::KkParams::default(),
    )
}

pub(super) fn run_drl_3d(topology: &Topology) -> Result<Geometry, StageError> {
    crate::layout::force::drl::run_3d(topology, &crate::layout::force::drl::DrlParams::default())
}

pub(super) fn run_fa2_3d(topology: &Topology) -> Result<Geometry, StageError> {
    crate::layout::forceatlas2::run_3d(topology, &crate::layout::forceatlas2::Fa2Params::default())
}

pub(super) fn run_yifan_2z(topology: &Topology) -> Result<Geometry, StageError> {
    crate::layout::force::yifan_hu::run_2z(topology, &crate::layout::force::ForceParams::default())
}

/// The 3D yifan_hu: [`crate::layout::force::yifan_hu::run_3d`] over the 3D multilevel
/// settle, at the frozen parameters — the same shim shape as `run_yifan_2z` above, so the
/// registry's function-pointer slot carries no dimension of its own.
pub(super) fn run_yifan_3d(topology: &Topology) -> Result<Geometry, StageError> {
    crate::layout::force::yifan_hu::run_3d(topology, &crate::layout::force::ForceParams::default())
}

// The six entries `LAYOUTS` appends after `layout.mds.pivot3d`, in this order (append only:
// the wasm module maps a layout by index).
pub(super) const YIFAN_HU_3D_LAYOUT: Capability = Capability {
    id: crate::layout::force::yifan_hu::ID_3D,
    run: run_yifan_3d,
    params: &LayoutParams::NONE,
    meta: YIFAN_HU_3D,
};
pub(super) const YIFAN_HU_2Z_LAYOUT: Capability = Capability {
    id: crate::layout::force::yifan_hu::ID_2Z,
    run: run_yifan_2z,
    params: &LayoutParams::NONE,
    meta: YIFAN_HU_2Z,
};
pub(super) const FRUCHTERMAN_REINGOLD_3D_LAYOUT: Capability = Capability {
    id: crate::layout::force::fruchterman_reingold::ID_3D,
    run: run_fr_3d,
    params: &LayoutParams::NONE,
    meta: FRUCHTERMAN_REINGOLD_3D,
};
pub(super) const KAMADA_KAWAI_3D_LAYOUT: Capability = Capability {
    id: crate::layout::force::kamada_kawai::ID_3D,
    run: run_kk_3d,
    params: &LayoutParams::NONE,
    meta: KAMADA_KAWAI_3D,
};
pub(super) const DRL_3D_LAYOUT: Capability = Capability {
    id: crate::layout::force::drl::ID_3D,
    run: run_drl_3d,
    params: &LayoutParams::NONE,
    meta: DRL_3D,
};
pub(super) const FA2_3D_LAYOUT: Capability = Capability {
    id: crate::layout::forceatlas2::ID_3D,
    run: run_fa2_3d,
    params: &LayoutParams::NONE,
    meta: FA2_3D,
};

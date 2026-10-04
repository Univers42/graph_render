//! Node overlap removal: push discs apart so no two nodes sit on each other.
//!
//! The user's report, 2026-10-03: *"sometimes there are too many nodes all compressed in the
//! same space, it's unreadable"*. Every layout in the tree places **points**, and nothing in
//! the pipeline separated discs, so a compressed layout shipped as it came out. `post::fdeb`
//! and `post::mingle` bundle edges and leave every node where the layout put it, and the one
//! collide term (`layout/force/params.rs`, `collide_radius`) is a force inside an iterative
//! solver — it never runs on a finished layout and is only reachable by asking for the live
//! force layout. So this pass exists.
//!
//! Reference, read for behaviour only: Gansner and Hu, "Efficient, Proximity-Preserving Node
//! Overlap Removal", JGAA 14(1) 2010 — PRISM — as implemented in Graphviz 16.1.0 at
//! `$GM_SCRATCH/refs/graphviz-16.1.0`, `lib/neatogen/overlap.c` (under `HAVE_GTS && SFDP`,
//! `overlap.c:17`) and `lib/neatogen/adjust.c`. EPL-1.0: read, never copied.
//!
//! # Why this is not PRISM
//!
//! PRISM needs a Delaunay triangulation. This crate takes no new dependency and has no
//! triangulation code to reuse — [`super::grid_index`] is a uniform grid, not a triangulator.
//! Writing Delaunay by hand would be a new subsystem with its own degenerate-case surface,
//! which is a worse trade than a grid sweep that can be audited end to end. So: **uniform-grid
//! sweeps**. `docs/decisions/node-overlap.md` §4 carries the reasoning in full.
//!
//! # The algorithm
//!
//! One radius per node (`radii`), then up to [`SeparateParams::max_iterations`] Jacobi
//! sweeps over a uniform grid ([`sweep`]). The cell side is `2 · (largest radius + margin)`,
//! which is what makes the 3 × 3 neighbourhood complete **by construction** rather than by
//! search. See `buckets` for the grid and [`sweep`] for the kernel.
//!
//! # Determinism (D1 to D10)
//!
//! `libm::sqrt` and arithmetic only — no `mul_add`, no `powi`, no relaxed SIMD. Every
//! reduction runs over the buckets, which a counting sort fills in ascending dense node order,
//! so the sum is fixed-order (D2, D4); nothing iterates a hash map. No clock, no RNG. `usize`
//! never on the wire. The same input gives the same bits on every target, native and wasm32
//! alike, which the hash gate checks four ways.
//!
//! # The z column
//!
//! **Refused, not carried.** This pass moves `x` and `y` and cannot reach `z`, so a geometry
//! carrying a z column is refused rather than half-processed: a 2D disc separation projected
//! under a z column answers a question nobody asked. Refusing it is what keeps
//! `docs/decisions/contract-3d-verdict.md` condition 6 — *no POST pass rewrites node columns*
//! — unamended. Three layouts emit z and are refused rather than damaged.
//!
//! # Why it is a POST pass at all
//!
//! POST's contract says a pass does not move nodes, and the composability matrix asserts that
//! over the whole registry. The claim is now a **declared field**,
//! [`Metadata::moves_nodes`](super::Metadata::moves_nodes), rather than an assumption: every
//! existing pass declares `false` and keeps the identical assert, and this one declares
//! `true`. The placement itself — a node-moving POST under a revised contract, over a new
//! stage after LAYOUT — was ruled on by the `devil` agent; see `docs/decisions/node-overlap.md`
//! §1 and §7.

mod buckets;
mod params;
mod radii;
pub mod sweep;

#[cfg(test)]
mod tests;

use crate::index::Topology;
use crate::layout::Geometry;
use crate::post::{Bundled, Metadata};
use crate::stage::StageError;
use graph_contract::geometry::{EdgeGeometry, EdgeGeometryKind};

pub use params::{SEPARATE_CEILING, SeparateParams, check};
pub use radii::{largest, of as radii_of};
pub use sweep::{TOLERANCE, Workspace};

/// The post capability's id, which is also its ledger row's and its hash-gate stage's.
pub const ID: &str = "post.separate.grid";

/// The pass's ledger row. Read across `docs/decisions/node-overlap.md` rather than restated:
/// `edges` is **pass-through**, because the pass emits whatever edges it was handed and there
/// is no `EdgeGeometryKind` for "the same edges".
pub const META: Metadata = Metadata {
    tier: 1,
    edges: EdgeGeometryKind::Line,
    moves_nodes: true,
    oracle: "hand: Gansner & Hu 2010 (PRISM, JGAA 14(1)) for the objective and the two-phase \
shape, read from Graphviz 16.1.0 lib/neatogen/overlap.c and adjust.c, EPL-1.0 and never \
copied. The algorithm is NOT PRISM: it is a uniform-grid Jacobi sweep, because PRISM needs a \
Delaunay triangulation this crate may not take a dependency for. Compared against that oracle \
on mean displacement and stress ratio only - a ceiling, never a bitwise match \
(docs/decisions/node-overlap.md 8). The invariant is checked by brute force in \
crates/graph-core/src/post/separate/tests.rs",
    complexity: "O(n x k x I): each of n nodes sweeps its 3x3 cell neighbourhood of k nodes, I <= \
max_iterations sweeps, and the early exit usually ends I well below the cap. Worst case k = n, \
when every disc shares one cell - which is the all-on-one-point adversarial case, so the \
degenerate input is the one that pays it. Counting sort and grid build are O(n + cells)",
    scale_ceiling: SEPARATE_CEILING,
    degradation: "past the ceiling the pass still returns finite geometry and never refuses - it \
stops fitting a one-second budget. The cell side is sized on the MAXIMUM node radius, so one \
huge node among a million small ones enlarges every cell and inflates k toward n; a caller with \
a heavy tail of node sizes pays in time, not in correctness. Bundled::unbundled is the count of \
pairs still overlapping by more than the tolerance, so a caller reads the residue instead of \
trusting the cap",
    ponytail: "Ponytail (iteration cap): the sweep is iterative, so it can stop with discs still \
touching - the all-on-one-point adversarial case needs more sweeps than a real layout. Failing \
input: n discs stacked on one point, n large enough that the cap binds. Direction: \
under-separates, never over-separates, and Bundled::unbundled counts exactly what is left. \
Escape hatch: SeparateParams::max_iterations. Ponytail (uniform grid): the cell side is 2 x \
(largest radius + margin), so a neighbourhood is sized by the largest node, not the median. \
Failing input: one node of radius R among n of radius r, R >> r. Direction: cost, not \
correctness - the big node's neighbourhood is near-global and k approaches n, so the pass slows \
down. Escape hatch: a size-stratified grid, which this is not. Ponytail (circumscribed Box): a \
Box is sized by its circumscribed radius, sqrt((w/2)^2 + (h/2)^2). Failing input: two boxes side \
by side sharing a bounding disc but no area. Direction: over-separates a Box layout - boxes end \
up further apart than they need, never closer, which is the safe direction for an invariant and \
the wrong one for fidelity to the reference, which also works in circles. Escape hatch: none; \
it is the price of one radius per node. Ponytail (coincident pair): two discs at exactly the \
same point have no separation direction, so the pair is pushed along x by half the \
penetration. Failing input: three or more discs at one point, which then stack along x and \
need further sweeps to spread on the y axis. Direction: under-separates, and the count of \
residual pairs reports it. Escape hatch: max_iterations. Ponytail (scale_ceiling): one timed \
run per size on one host, no warm-up, release build (docs/measurements/ux-overlap.md)",
};

/// The pass at its default parameters — the run [`ID`] is registered and hashed under.
pub fn run(topology: &Topology, geometry: &Geometry) -> Result<Bundled, StageError> {
    separate(topology, geometry, &SeparateParams::default())
}

/// Separates `geometry`'s node discs at `params`.
///
/// **The only POST entry point that may move a node**, which is why it goes through
/// [`Geometry::with_nodes`] rather than [`Geometry::with_edges`] — see this module's doc and
/// `docs/decisions/node-overlap.md` §3. Edges, notes and the z refusal are carried through
/// unchanged: a `Line`'s endpoints come from node geometry, so moved nodes redraw their edges
/// with no edge column touched.
pub fn separate(
    topology: &Topology,
    geometry: &Geometry,
    params: &SeparateParams,
) -> Result<Bundled, StageError> {
    check(geometry, params)?;
    let (nodes, pairs, unbundled) = sweep_nodes(geometry, params)?;
    if geometry.nodes.check(topology.node_count(), None).is_err() {
        return Err(StageError::Param {
            name: "node count",
            rule: "the topology's own",
        });
    }
    Ok(Bundled {
        geometry: geometry.with_nodes(nodes),
        pairs,
        unbundled,
    })
}

/// The moved node geometry, and the two counts [`separate`] reports: overlapping pairs a
/// sweep resolved, and pairs still overlapping by more than [`sweep::TOLERANCE`] once the
/// cap was reached.
///
/// Split out so [`separate`] stays a function of three parameters and one rule.
fn sweep_nodes(
    geometry: &Geometry,
    params: &SeparateParams,
) -> Result<(graph_contract::geometry::NodeGeometry, u32, u32), StageError> {
    let radii = radii::of(&geometry.nodes, params.point_radius)?;
    let mut work = sweep::Workspace::new(radii.len());
    let run = sweep::Run {
        margin: params.margin as f32,
        omega: params.over_relaxation,
        max_iterations: params.max_iterations,
    };
    let (pairs, unbundled) = work.sweep(&geometry.nodes, &radii, run)?;
    let (x, y) = work.positions();
    Ok((rebuild(&geometry.nodes, x, y), pairs, unbundled))
}

/// The same node kind as the input, with the swept positions in it. The kind is never
/// changed: a pass that turned `Circle`s into `Point`s would be silently downgrading the
/// drawing, which is what the `Line`/`Circle`/`Box` contract exists to prevent.
fn rebuild(
    nodes: &graph_contract::geometry::NodeGeometry,
    x: &[f32],
    y: &[f32],
) -> graph_contract::geometry::NodeGeometry {
    use graph_contract::geometry::NodeGeometry;
    match nodes {
        NodeGeometry::Point { .. } => NodeGeometry::Point {
            x: x.to_vec(),
            y: y.to_vec(),
        },
        NodeGeometry::Circle { r, .. } => NodeGeometry::Circle {
            x: x.to_vec(),
            y: y.to_vec(),
            r: r.clone(),
        },
        NodeGeometry::Box { w, h, .. } => NodeGeometry::Box {
            x: x.to_vec(),
            y: y.to_vec(),
            w: w.clone(),
            h: h.clone(),
        },
    }
}

/// The pass emits the edges it was handed; named here so a ledger reader is not left
/// wondering whether `META::edges` means something else for this row.
pub const EDGES_ARE_PASS_THROUGH: bool = true;

#[allow(dead_code)]
fn assert_edge_kind_is_named(edges: &EdgeGeometry) -> EdgeGeometryKind {
    edges.kind()
}

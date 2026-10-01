//! Ledger metadata for the four closed-form networkx layouts (`layout.random`,
//! `layout.circular.ring`, `layout.spiral`, `layout.bipartite`), kept apart from
//! `registry.rs` for the house line cap.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count these four layouts were run at, the largest size `graph-cli bench` accepts
/// (`docs/measurements/tier1-scale.md`). At 1 000 000 nodes and 1 549 929 edges the
/// registered run takes 9.2 ms (random), 13.2 ms (ring), 32.0 ms (spiral) and 272 ms
/// (bipartite), native release. Nothing was run above it.
///
/// Ponytail (scale_ceiling): a measured lower bound, not the wall: the layouts are linear,
/// so the memory wall the grid's 4 600 000 projects onto wasm32 is far above what was run,
/// and this figure understates it. Timings are `--repeat 3` medians on one host.
pub const CLOSED_FORM_CEILING: u64 = 1_000_000;

const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes — a refusal, never a \
wrap or a truncation";

pub(super) const RANDOM: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx@3.6 random_layout — shape only (uniform points in [0,1)^2, x then y per \
node); the stream is the crate's Mulberry32 at a fixed seed, so coordinates are pinned by unit \
test, not compared against numpy's generator",
    complexity: "O(n)",
    scale_ceiling: CLOSED_FORM_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail: the stream is not numpy's Mersenne Twister, so no coordinate equals \
networkx's for any seed; only the distribution is reproduced. Direction: none visible, a random \
layout has no correct answer. Ponytail (scale_ceiling): a measured lower \
bound — see CLOSED_FORM_CEILING.",
};

pub(super) const RING: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx@3.6 circular_layout — node k at angle 2*pi*k/n in dense-index order, then \
rescale_layout; compared by harness/oracle-closed-form.py within 1e-6 (networkx narrows theta to \
f32)",
    complexity: "O(n)",
    scale_ceiling: CLOSED_FORM_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail: angles are f64 where networkx narrows them to f32, so the differential \
is a 1e-6 tolerance, never bytes; the layout itself is exact. Ponytail (scale_ceiling): \
a measured lower bound — see CLOSED_FORM_CEILING.",
};

pub(super) const SPIRAL: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx@3.6 spiral_layout, 2D, at its defaults (resolution 0.35, equidistant \
false; SciGraphs' 2D dispatcher passes none) — compared by harness/oracle-closed-form.py \
within 1e-7",
    complexity: "O(n)",
    scale_ceiling: CLOSED_FORM_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail: the parameters are networkx's defaults, not a SciGraphs setting, since \
SciGraphs has no 2D spiral; a caller wanting the equidistant curve uses run_with. Ponytail \
(scale_ceiling): a measured lower bound — see CLOSED_FORM_CEILING.",
};

pub(super) const BIPARTITE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx@3.6 bipartite_layout (vertical, aspect 4/3) given SciGraphs' node sets \
(hierarchical.py _bipartite_parts); compared by harness/oracle-closed-form.py within 1e-7 given \
our own node sets (the partition rule itself is not compared)",
    complexity: "O(n + m)",
    scale_ceiling: CLOSED_FORM_CEILING,
    degradation: "a graph that does not two-colour (odd cycle, self-loop) is split by a greedy \
maximum cut refined by 8 passes and drawn anyway, some edges then running inside a column; it \
never panics or refuses. Past the ceiling wasm32 cannot allocate and the module traps",
    ponytail: "Ponytail (non-bipartite fallback): the greedy cut is a 1/2-approximation, not \
the maximum; on a non-bipartite graph edges inside a set draw as vertical lines in one column — \
cosmetic. Ponytail (scale_ceiling): a measured lower \
bound — see CLOSED_FORM_CEILING.",
};

/// The 3D closed-form arms' ceiling: the 2D family's, re-measured for the 3D run in
/// `docs/measurements/p12-t4a.md`. Every one of these is O(n) with a per-node gather, so
/// the 3D rows measure within the same order of magnitude; the z column adds 4 bytes a node.
pub const CLOSED_FORM_3D_CEILING: u64 = CLOSED_FORM_CEILING;

const DEGRADATION_3D: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity \
once an id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation. The \
z column is a third f32 per node, so the same node count costs about 4 bytes more than its 2D \
counterpart before anything else";

pub(super) const RANDOM_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx@3.6 random_layout at dim=3 — shape only (uniform points in [0,1)^3, x \
then y then z per node); the stream is the crate's Mulberry32 at a fixed seed, so coordinates \
are pinned by unit test, not compared against numpy's generator",
    complexity: "O(n)",
    scale_ceiling: CLOSED_FORM_3D_CEILING,
    degradation: DEGRADATION_3D,
    ponytail: "Ponytail: the stream is not numpy's Mersenne Twister, so no coordinate equals \
networkx's for any seed; only the distribution is reproduced. Direction: none visible, a random \
layout has no correct answer. Ponytail (scale_ceiling): a measured lower \
bound — see CLOSED_FORM_3D_CEILING.",
};

pub(super) const SPIRAL_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs basic.py _spiral_layout_3d:36-63 — a conical spiral of radius \
scale*0.5..scale, its nodes at even ARC spacing via the reference's own 65536-sample trapezoid \
integral inverted per node (turns = max(2, round(sqrt(n / (0.75*pi)))), omega = 2*pi*turns, \
z = scale*(2t - 1)); compared by harness/oracle-closed-form.py. NOT networkx: spiral_layout \
refuses dim=3 (drawing/layout.py:1315, 'can only handle 2 dimensions'), and this curve is not \
layout.spiral at another dimension — layout.spiral is networkx's Archimedean spiral, and its own \
header records that SciGraphs' 2D dispatcher never calls it",
    complexity: "O(n + 65536): the arc table is a fixed 65536-sample integral whatever n is, so \
a small graph is not cheaper than a large one",
    scale_ceiling: CLOSED_FORM_3D_CEILING,
    degradation: DEGRADATION_3D,
    ponytail: "Ponytail: the arc table is 65536 f64 (1 MiB) rebuilt per run, so this id's cost is \
set by the table and not by the node count. Failing input: a one-node graph, which takes the \
reference's wanted = 0.5 * length[-1] branch and lands mid-arc — ported verbatim. Direction: the \
table is a fixed-resolution integral, so a caller passing a very large turns under-resolves the \
fastest part of the cone and the spacing drifts toward even-in-t; cosmetic. Escape hatch: \
spiral::spiral_3d::spiral_3d_with takes turns directly, and the registered run is pinned to \
the reference default",
};

pub(super) const BIPARTITE_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs hierarchical.py _bipartite_layout_3d:213-242 — the two node sets from \
_bipartite_parts (the same partition module layout.bipartite uses) on parallel planes z = \
∓scale*0.5, each set on a ring of radius scale*0.6 at angle (i / max(1, count)) * 2*pi; \
compared by harness/oracle-closed-form.py. The partition is shared with the 2D arm; the \
placement is not, which is why this is not a dims parameter over layout.bipartite",
    complexity: "O(n + m)",
    scale_ceiling: CLOSED_FORM_3D_CEILING,
    degradation: "a graph that does not two-colour (odd cycle, self-loop) is split by the shared \
greedy maximum cut and drawn anyway, some edges then running inside a ring; it never panics or \
refuses. Past the ceiling wasm32 cannot allocate and the module traps",
    ponytail: "Ponytail (non-bipartite fallback): the greedy cut is a 1/2-approximation, not the \
maximum, so on a non-bipartite graph some edges run inside a ring — cosmetic. Ponytail (no \
rescale): the reference places at its default scale = 1 and does not _rescale_positions, so \
unlike layout.bipartite this arm's output is NOT rescaled to the unit square — the rings sit \
at radius 0.6 and the planes at z = ∓0.5 whatever the node count, which is faithful to the \
reference but means the two arms are not scaled alike. Ponytail (scale_ceiling): a measured \
lower bound — see CLOSED_FORM_3D_CEILING",
};

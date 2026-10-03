//! Ledger metadata for the three pipeline layouts the registry declares here: the grid,
//! circle packing and the layered drawing.
//!
//! Split out of `registry.rs` by the house's 300-line limit. Each `Metadata` carries its
//! ceiling's whole derivation in the doc above the constant, and none of the three belongs
//! to a family the other `registry/*.rs` modules table (no second layout shares a control,
//! an oracle arm or a knob), so a file of its own is the split the concern already has.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count past which `layout.grid` stops being usable, and why it is this one.
///
/// Estimated, not measured on the target (`crates/graph-core/tests/memory.rs`,
/// `grid_pipeline_memory_per_node`): natively, the topology stage, the grid and the
/// snapshot's bytes peak at **919 B per node** at 100 000 synthetic nodes and 154 978
/// edges (91.9 MB), input records excluded. wasm32 addresses at most 4 GiB, so
/// 4 GiB / 919 B = 4.67 M nodes, rounded down to two figures. The grid's own `u32` limits bind far later: its lattice is exact
/// up to 2^32 − 1 nodes, and the id tables refuse past 2^32 − 1 bytes of text.
pub const GRID_CEILING: u64 = 4_600_000;

pub(super) const GRID: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "hand: the conventions worked by hand in graph-core layout/grid.rs, restated in f64 \
and checked per seed by graph-cli roundtrip; no third-party grid is a meaningful oracle \
(SciGraphs' _grid_layout fits its scale and starts at the origin)",
    complexity: "O(n)",
    scale_ceiling: GRID_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "Ponytail (aspect): cols = ceil(sqrt(n)) is a convention, not a computation; when \
cols does not divide n the last row is ragged and the nodes' centroid sits off the origin (n = 3: \
(-1/6, -1/6)). Direction: cosmetic, never wrong — every node gets its own cell. Escape hatch: a \
layout that centres the last row, under its own id. Ponytail (scale_ceiling): an estimate — \
measured natively on 64-bit and projected onto wasm32's 4 GiB; re-measure with \
crates/graph-core/tests/memory.rs",
};

/// Node count past which `layout.packing.circle` stops being usable, and why it is this
/// one — a different shape of ceiling than the other three, and much lower.
///
/// Labelled, not a hard memory wall: the exact Collins–Stephenson path (genuinely planar
/// input) is close to linear in `n`, like the other three layouts. But a random or dense
/// graph at synthetic-model density is essentially always non-planar (the planar bound is
/// `m <= 3n - 6`), so the realistic case takes `circle_packing/fallback.rs`'s two O(n^2)
/// relaxation passes. Measured natively, `--release`
/// (`crates/graph-core/tests/memory.rs::circle_packing_pipeline_memory_per_node`): one
/// pipeline call takes 304 ms at n=300, 2.81 s at n=1000, 22.63 s at n=3000 — the O(n^2)
/// shape shows in the timing and in peak memory, which does *not* hold flat per node the
/// way the other three layouts' does (3.2 KB/node at n=300 rising to 24.9 KB/node at
/// n=3000). Fitting that quadratic, a single call already crosses a 1-second budget
/// around n=600. 5,000 is a round, stated cutoff at which a fallback packing already
/// costs tens of seconds even natively; it is not measured directly at that size because
/// doing so is itself impractically slow — the same reason this ceiling exists.
pub const PACKING_CEILING: u64 = 5_000;

pub(super) const PACKING: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Circle,
    edges: EdgeGeometryKind::Line,
    oracle: "hand + planarity certificate: the Collins-Stephenson exact path is checked against \
its own Euler-formula certificate (planarity::planar_embedding + triangulate_embedding); the \
per-seed hand oracle (graph-cli roundtrip) restates finite radii, no NaN/Inf, and edge tangency \
within tolerance whenever note code 3 is absent — no third-party packer is pinned to this exact \
convention",
    complexity: "O(n) exact path; O(n^2) per relaxation round on the non-planar fallback",
    scale_ceiling: PACKING_CEILING,
    degradation: "past the ceiling the fallback still runs and still returns finite geometry, \
never a refusal or a trap — it simply gets slower at O(n^2), with no built-in cutoff, so a \
caller must apply its own timeout; the exact planar path is unaffected and stays fast at any n \
this crate's u32 index space allows",
    ponytail: "the packing is exact only for planar input. The failing input is any graph with a \
K5 or K3,3 minor (or one whose planar embedding cannot be triangulated into a genuine disk, \
treated the same defensively). Direction: overlap, the dangerous one — the fallback does not \
guarantee tangency or non-overlap either. Escape hatch: read note code 3 off the snapshot; its \
absence is the only trustworthy sign the packing is exact (module doc; full account in \
docs/decisions/planarity-fallback.md). Ponytail (scale_ceiling): labelled, time-bound, not \
measured at the ceiling itself — see PACKING_CEILING's derivation.",
};

/// Layered-vertex count past which `layout.dag.sugiyama` routes no more long arcs: the
/// reference's own `_DUMMY_BUDGET` (`SciGraphs/.../hierarchical.py:7`).
pub const SUGIYAMA_CEILING: u64 = 200_000;

pub(super) const SUGIYAMA: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Polyline,
    oracle: "dagre-d3-es 7.0.14 crossing counts (harness/oracle-layouts.mjs --dag, margin frozen \
in docs/measurements/phase05-crossings.md) and SciGraphs hierarchical.py; per-seed structural \
invariants (acyclic after orienting every non-loop edge forward along the dense node order, \
monotone layers, contiguous dummy chains) checked by graph-cli roundtrip; a self-loop is \
emitted with no interior points, as the reference's own layout stage emits none (it returns \
node positions only, hierarchical.py:651-652) — the loop arc is the router's, drawn from the \
coincident endpoints (edge_styles.py:458)",
    complexity: "one O(m log m) sort of the arc list up front, then O(n+m) per phase except \
crossing reduction, which sorts each layer's median keys and the transpose's neighbour \
positions once per vertex per sweep, O((n+m) log n) a sweep; crossing reduction is a heuristic \
(median + transpose local search), not a minimiser",
    scale_ceiling: SUGIYAMA_CEILING,
    degradation: "past the dummy budget (200000) long arcs are left straight and unrouted and \
each is reported as note 4 dag.dummy_budget_exceeded; above 150000 layered vertices the transpose \
rounds drop to 0, so crossings rise while the drawing stays valid; above 200000 layered \
vertices the whole X phase is skipped as well and x stays the raw ordering slot index — legal, \
maximally spread, and reported here rather than as a note, because the notes section is a \
closed set (codes 1-5) with no code for a skipped phase and one that would have to be \
snapshot-wide",
    ponytail: "Ponytail (crossing reduction): median + transpose is a local search; a graph \
whose optimal order it cannot reach draws more crossings than optimal — cosmetic, never \
incorrect. Ponytail (dummy budget): an unrouted long arc is a straight line that may pass \
through nodes — visually wrong, the dangerous direction; escape hatch: read note 4 in the \
snapshot. Ponytail (cycle breaking): arcs are oriented along the dense node order, not a greedy \
feedback-arc-set peel, so a cycle is broken at every backwards edge rather than the fewest \
possible — extra reversed edges (note 5) are cosmetic, each one drawn head to tail rather \
than dropped",
};

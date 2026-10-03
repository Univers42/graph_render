//! Ledger metadata for the four hierarchy rows (`layout.tree.tidy`,
//! `layout.treemap.squarified`, `layout.circular.radial` and
//! `layout.circular.hierarchy`), kept apart from `registry.rs` for the house line cap.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count past which the four hierarchy rows stop being usable, and why it is this one.
///
/// **One constant, four rows, and a measurement of three.** `layout.tree.tidy`,
/// `layout.treemap.squarified`, `layout.circular.radial` and `layout.circular.hierarchy`
/// all quote this figure; the first three are measured, the fourth is named at the end and
/// each row repeats the disclosure in its own `ponytail`.
///
/// Measured, not estimated (`crates/graph-core/tests/memory.rs`,
/// `hierarchy_layout_pipeline_memory_per_node`): each of the three holds within a few
/// percent of the grid's own 919 B/node at 100 000 synthetic nodes and 154 978 edges —
/// tidy tree 933 B, treemap 930 B, circular 922 B/node — because all three add only an O(n)
/// hierarchy repair (`layout/hierarchy.rs`) and O(n) geometry over the same topology and
/// snapshot substrate the grid does. wasm32 addresses at most 4 GiB, so 4 GiB / 933 B =
/// 4.60 M nodes at the heaviest of the three (tidy tree), rounded down to two figures, same
/// as `GRID_CEILING`. The hierarchy repair's own `u32` limit binds far later:
/// `Hierarchy::of` needs `n + 1` rows to fit `u32`, i.e. up to 2^32 − 2 nodes.
///
/// **The fourth row rides the tidy-tree figure and has none of its own.**
/// `layout.circular.hierarchy` is a port of SciGraphs' own ring layout over the same
/// `_component_roots` and `_multi_source_levels` repair the other three run, so it reads the
/// same topology, keeps the same O(n) output columns and allocates nothing per node the
/// others do not. That is why one ceiling can stand for four rows; it is not a measurement
/// of the fourth — `hierarchy_layout_pipeline_memory_per_node` sweeps `layout.tree.tidy`,
/// `layout.treemap.squarified` and `layout.circular.radial`, and not this one.
///
/// Ponytail (scale_ceiling): what it gets wrong — the fourth row's per-node cost is assumed
/// equal to its siblings' rather than measured, so if it ever grew a per-node scratch the
/// 933 B would be stale and the ceiling optimistic. Direction: too low, never too high, and
/// the three measured rows span 922-933 B — 1.2% end to end at 100 000 nodes — so the shared
/// figure has little room to be wrong. Escape hatch: add the row to that one test and
/// re-derive the division.
pub const HIERARCHY_LAYOUT_CEILING: u64 = 4_600_000;

/// The ring layout SciGraphs itself ships, `CIRCULAR_HIERARCHY`
/// (`hierarchical.py:693-732`), compared coordinate by coordinate against the reference
/// function itself.
///
/// The module doc of `layout/circular/hierarchy.rs` carries the third-of-three
/// disambiguation, and this row's oracle the measurement: 1000 seeds, worst
/// `max |ours - theirs|` 2.380e-7, ceiling 1e-6, and every one of the 1000 seeds equal after
/// narrowing both arms to `f32` (`docs/measurements/p12-t2.md`).
pub(super) const CIRCULAR_HIERARCHY: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _circular_hierarchy_layout (hierarchical.py:693-732) at the \
    dispatcher's own scale=5.0 (dispatcher.py:14), ported whole: _component_roots \
    (hierarchical.py:31-52), _multi_source_levels (:54-81), the max(level, 0.35) * scale / \
    max(2, max_level) radius (:723-726) and the (i/count) * 2 * pi angle (:728). Compared \
    against the SciGraphs arm itself in the ge-python-oracle image with the submodule \
    mounted, over 1000 seeds, by harness/oracle-circular-hierarchy.py, within 1e-6 — the \
    next power of ten above the worst measured gap, which is the snapshot's f32 narrowing \
    and nothing else. The one departure: a directed input is read undirected, because the \
    motor's Topology has per-edge directedness and no whole-graph flag, so the port takes \
    the _component_roots branch and the SciGraphs arm is handed an nx.Graph to match. \
    Distinct from layout.circular.radial, which declines this very comparison (see the \
    hand oracle above) and is a different function, and from layout.circular.ring, which \
    reads no structure at all and shares with this one only the word 'circular'",
    complexity: "O(n + m): two BFS sweeps per component for the roots, one multi-source BFS \
    for the levels, one pass to place",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
    result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity \
    once an id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a \
    truncation. The layout itself never refuses: SciGraphs' own rules are total, so there \
    is no input past which this module's own answer stops existing",
    ponytail: "No Ponytail on the algorithm: this is a whole port of a closed form with no \
    RNG and no iteration, so nothing here is a heuristic, an estimate or a fallback, and \
    none is owed (module doc). The departure is stated rather than owed: a digraph is read \
    undirected, so SciGraphs' in-degree-0 roots and its three-biggest-fan-outs fallback \
    (hierarchical.py:705-711) are not ported. Direction: a different root set, hence a \
    different level assignment, hence a different drawing — cosmetic, and the alternative \
    would be inventing a whole-graph directedness rule the contract does not have. Escape \
    hatch: repair the digraph into a forest upstream, where the caller already owns the \
    direction. Ponytail (scale_ceiling): measured for the hierarchy family, not for this \
    module alone — see HIERARCHY_LAYOUT_CEILING's derivation",
};

pub(super) const TIDY_TREE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Polyline,
    oracle: "d3-hierarchy@3.1.2 tree() — an exact f64 port of tree.js's Buchheim/Jünger/ \
Leipert/Walker algorithm at d3's own defaults (size([1,1]), the default separation), stated in \
the module doc; byte-compared after Math.fround by harness/oracle-layouts.mjs over >=1000 seeds",
    complexity: "O(n)",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "No Ponytail on the algorithm: the port is exact, nothing here is a heuristic, an \
estimate or a fallback, so none is owed (module doc). Ponytail (scale_ceiling): measured, not \
estimated — see HIERARCHY_LAYOUT_CEILING's derivation and \
crates/graph-core/tests/memory.rs::hierarchy_layout_pipeline_memory_per_node.",
};

pub(super) const TREEMAP: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Box,
    edges: EdgeGeometryKind::Line,
    oracle: "d3-hierarchy@3.1.2 treemap().tile(treemapSquarify) — an exact f64 port of \
squarify.js and hierarchy.sum at d3's own defaults (size([1,1]), no padding, no rounding), \
stated in the module doc; byte-compared after Math.fround by harness/oracle-layouts.mjs over \
>=1000 seeds",
    complexity: "O(n log n)",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "a non-positive or non-finite weight clamps to WEIGHT_EPSILON (1e-6) rather than \
vanishing or handing squarify a zero/NaN value — the oracle applies the identical clamp. \
Direction: cosmetic under-representation, a hairline never a wrong containment; escape hatch: \
fix the weight upstream (module doc). Ponytail (scale_ceiling): measured, not estimated — see \
HIERARCHY_LAYOUT_CEILING's derivation and \
crates/graph-core/tests/memory.rs::hierarchy_layout_pipeline_memory_per_node.",
};

pub(super) const CIRCULAR: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "hand: the ring/angle/radius conventions worked by hand in graph-core \
layout/circular.rs (docs/decisions/circular-conventions.md), restated independently in f64 and \
checked per seed by graph-cli roundtrip; no third-party circular/radial hierarchy layout is a \
meaningful byte-for-byte oracle (SciGraphs' own hierarchical.py normalises differently, per that \
decision doc)",
    complexity: "O(n + m): Hierarchy::of reads every topology edge (keep_lowest_parent_edges, \
break_cycles) before the ring pass, which reads no structure itself — the CIRCULAR_HIERARCHY \
row above says the same for the same family",
    scale_ceiling: HIERARCHY_LAYOUT_CEILING,
    degradation: "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, the snapshot refuses with SnapshotError::Capacity once an \
id table's text would pass 2^32-1 bytes — a refusal, never a wrap or a truncation",
    ponytail: "the radius step and the start angle are conventions pinned by this module, not a \
computation with one right answer (docs/decisions/circular-conventions.md). Failing input: a \
ring holding many nodes at a small radius (a shallow, bushy tree) crowds them close together. \
Direction: cosmetic, never wrong — every node keeps its own ring and a distinct slot, so no two \
real nodes ever collide. Escape hatch: a variant that inflates the radius by ring population, \
under its own id (module doc). Ponytail (scale_ceiling): measured, not estimated — see \
HIERARCHY_LAYOUT_CEILING's derivation and \
crates/graph-core/tests/memory.rs::hierarchy_layout_pipeline_memory_per_node.",
};

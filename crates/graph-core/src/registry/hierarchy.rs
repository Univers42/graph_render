//! Ledger metadata for the three hierarchy layouts (`layout.tree.tidy`,
//! `layout.treemap.squarified`, `layout.circular.radial`), kept apart from `registry.rs`
//! for the house line cap.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count past which the three hierarchy layouts (tidy tree, treemap, circular) stop
/// being usable, and why it is this one.
///
/// Measured, not estimated (`crates/graph-core/tests/memory.rs`,
/// `hierarchy_layout_pipeline_memory_per_node`): each holds within a few percent of the
/// grid's own 919 B/node at 100 000 synthetic nodes and 154 978 edges — tidy tree 933 B,
/// treemap 930 B, circular 922 B/node — because all three add only an O(n) hierarchy
/// repair (`layout/hierarchy.rs`) and O(n) geometry over the same topology and snapshot
/// substrate the grid does. wasm32 addresses at most 4 GiB, so 4 GiB / 933 B = 4.60 M
/// nodes at the heaviest of the three (tidy tree), rounded down to two figures, same as
/// `GRID_CEILING`. The hierarchy repair's own `u32` limit binds far later:
/// `Hierarchy::of` needs `n + 1` rows to fit `u32`, i.e. up to 2^32 − 2 nodes.
pub const HIERARCHY_LAYOUT_CEILING: u64 = 4_600_000;

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
    complexity: "O(n)",
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

//! Ledger metadata for the four closed-form networkx layouts (`layout.random`,
//! `layout.circular.ring`, `layout.spiral`, `layout.bipartite`), kept apart from
//! `registry.rs` for the house line cap.

use super::{GRID_CEILING, Metadata};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

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
    scale_ceiling: GRID_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail: the stream is not numpy's Mersenne Twister, so no coordinate equals \
networkx's for any seed; only the distribution is reproduced. Direction: none visible, a random \
layout has no correct answer. Ponytail (scale_ceiling): estimated, not measured — see \
crates/graph-core/src/registry/closed_form.rs.",
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
    scale_ceiling: GRID_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail: angles are f64 where networkx narrows them to f32, so the differential \
is a 1e-6 tolerance, never bytes; the layout itself is exact. Ponytail (scale_ceiling): \
estimated, not measured — see crates/graph-core/src/registry/closed_form.rs.",
};

pub(super) const SPIRAL: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx@3.6 spiral_layout, 2D, at its defaults (resolution 0.35, equidistant \
false; SciGraphs' 2D dispatcher passes none) — compared by harness/oracle-closed-form.py \
within 1e-6",
    complexity: "O(n)",
    scale_ceiling: GRID_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail: the parameters are networkx's defaults, not a SciGraphs setting, since \
SciGraphs has no 2D spiral; a caller wanting the equidistant curve uses run_with. Ponytail \
(scale_ceiling): estimated, not measured — see crates/graph-core/src/registry/closed_form.rs.",
};

pub(super) const BIPARTITE: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "networkx@3.6 bipartite_layout (vertical, aspect 4/3) given SciGraphs' node sets \
(hierarchical.py _bipartite_parts); compared by harness/oracle-closed-form.py within 1e-6, the \
sets restated independently in the oracle",
    complexity: "O(n + m)",
    scale_ceiling: GRID_CEILING,
    degradation: "a graph that does not two-colour (odd cycle, self-loop) is split by a greedy \
maximum cut refined by 8 passes and drawn anyway, some edges then running inside a column; it \
never panics or refuses. Past the ceiling wasm32 cannot allocate and the module traps",
    ponytail: "Ponytail (non-bipartite fallback): the greedy cut is a 1/2-approximation, not \
the maximum; on a non-bipartite graph edges inside a set draw as vertical lines in one column — \
cosmetic. Ponytail (scale_ceiling): estimated, not measured — see \
crates/graph-core/src/registry/closed_form.rs.",
};

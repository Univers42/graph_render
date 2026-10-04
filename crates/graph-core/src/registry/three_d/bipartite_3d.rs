//! Ledger metadata for `layout.bipartite_3d`, kept apart from [`super`] for the house
//! line cap — that file was already at 296 lines with five layouts in it.
//!
//! **The one entry here that is not in [`super`]'s closed-form family, and the placement is
//! the difference.** `SPHERE`, `HELIX` and `CUBE` read a node count; this reads a graph, and
//! then reads the node sets SciGraphs' own `_bipartite_parts` reads. It borrows that rule
//! from `layout::bipartite` rather than re-porting it, so it inherits one heuristic with it:
//! the greedy maximum cut a non-bipartite graph falls back on. That heuristic is the
//! [`Metadata::ponytail`] field, and it is the only approximate thing in the layout.

use super::super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// The same refusal `super::DEGRADATION` states, written for this layout alone.
///
/// **Not `super::DEGRADATION`, and the reason is blast radius rather than wording.** That
/// string is the `degradation` field of six other rows, and the repair-job rule is to edit
/// only your own row's lines; rewording a shared constant to change its count from five to
/// six would move five other capabilities' published metadata to say one word more about a
/// layout that did not change. One string, restated, beats that.
const DEGRADATION: &str = "past the ceiling wasm32 cannot allocate and the module traps (no \
partial result); natively, memory permitting, the snapshot refuses with \
SnapshotError::Capacity once an id table's text would pass 2^32-1 bytes, and with \
SnapshotError::Length { column: \"node.z\" } if the z column does not match the node count — a \
refusal, never a wrap or a truncation. This layout refuses for no input of its own: \
_bipartite_parts either returns two sets or None, the greedy cut always returns two, they \
between them cover every node exactly once, and every branch of both is total";

/// `pub(in crate::registry)`, not `pub(super)`: the siblings are declared in `three_d.rs`
/// where `pub(super)` already means the registry, and this one is a child module, where the
/// same word would mean `three_d` only and stop one level short.
pub(in crate::registry) const BIPARTITE_3D: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "SciGraphs' own _bipartite_layout_3d \
(SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:213-242) at scale = 5.0, over its \
own _bipartite_parts (:149-181) and _greedy_max_cut (:183-211) — the reference function \
itself, not a library layout. Compared as conformance row BIPARTITE_3D by \
scripts/scigraphs-conformance.sh over the 24 conformance fixtures (1020 coordinates): the \
f32 column matches 1020 of 1020 exactly, and the f64 column does not, because numpy's cos \
and sin are its own kernels and differ from libm's in the last ulp on some arguments (the \
sg-common caveat), so the row's tier is `tolerance` and its cause `arithmetic`. \
**This is not layout.bipartite's oracle and shares none of it:** that row is networkx's \
bipartite_layout, two vertical columns in a rescaled unit box, compared within 1e-7 given \
our own node sets, while this row is two horizontal rings at z = +/-scale*0.5 and is not \
rescaled. They share the two node sets and nothing else",
    complexity: "O(n + m): one two-colouring BFS, or one greedy cut plus at most 8 \
single-vertex passes, then two libm transcendentals per node and no iteration",
    scale_ceiling: super::BASIC_3D_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail: the failing input is any graph with an odd cycle or a self-loop — \
_bipartite_parts gives up on the WHOLE graph at the first one it meets \
(hierarchical.py:174-175) and _greedy_max_cut (:183-211) guarantees only that half the edges \
run between the planes, so some edges end up inside a ring and are drawn as chords in one \
plane. Direction: COSMETIC and loud rather than silent: a chord across a ring reads as a \
chord, where the previous mapping (layout.bipartite, two columns) drew the same edge as a \
vertical line that looked like structure that was not there. The approximation is the cut \
itself and it is not ours — a graph with two good cuts and no colouring gets one of them \
by construction, never the maximum cut, and a self-loop is dropped from the count entirely \
(:192, :202). Escape hatch: none is needed, and none should be added: a bipartite graph \
never reaches the heuristic, and the rest of this layout is exact",
};

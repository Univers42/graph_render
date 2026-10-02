//! The registry the ledger projects: one [`StyleCapability`] per style, each carrying the
//! full [`Metadata`] a `prompt.md` §8 row needs — tier, stage, geometry, oracle,
//! complexity, `scale_ceiling`, `degradation` and `ponytail`, none of them optional, so a
//! style cannot be registered without declaring what it costs and what it owes. Split
//! from the parent for the house line limit; the parent's module doc carries the
//! geometry.
//!
//! The four rows differ in exactly one field, `edges`. A style never emits two geometry
//! kinds, so a row can name one, and everything else is one answer for all four.

use super::Style;
use crate::registry::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count past which a POST style stops being usable, and why it is this one.
///
/// Estimated, not measured on the target: a style adds at most two interior points per
/// edge — 16 B of `f32` — plus one `u32` offset per edge, so at the synthetic model's
/// 1.55 edges per node that is **31 B per node** on top of the topology layer's measured
/// 442 B per node (`docs/measurements/p1-topology-memory.md`) for **473 B per node**. wasm32
/// addresses at most 4 GiB, so 4 GiB / 473 B = 9.08 M nodes, rounded down to two figures.
/// The style's own `u32` limit binds far later: a self-loop row is the longest at
/// `self_loop_segments` points, and 2^32 - 1 points across a graph is hundreds of
/// millions of edges.
///
/// Ponytail (loop rows): the synthetic model has no self-loops, and a loop row holds up to
/// 32 points (256 B), so on a loop-heavy graph this over-states the ceiling by up to 8x.
/// Re-measure with `crates/graph-core/tests/memory.rs` on such a graph.
pub const POST_STYLE_CEILING: u64 = 9_100_000;

const ORACLE: &str = "hand: the conventions stated in the parent module's doc, restated in f64 and \
pinned per generator by graph-core post/styles/tests.rs and at every CSR boundary by \
graph-core/tests/edge_geometry_invariants.rs; SciGraphs' edge_styles.py is read as a \
reference, not an oracle — it bakes style points into a 3-D mesh vertex list, so there is no \
2-D CSR to byte-compare against (the same division docs/decisions/circular-conventions.md \
draws for the radial layout)";

const COMPLEXITY: &str = "O(n + m x s), s <= 32: two stable counting sorts over 0..n for the fan, \
then one pass over the edges, each writing its own row of s points from the two node columns — at \
most 2, or self_loop_segments (3..=32, params::MAX_LOOP_SEGMENTS) for a loop";

const DEGRADES: &str = "past the ceiling wasm32 cannot allocate and the module traps (no partial \
result); natively, memory permitting, style_edges refuses with StageError::Capacity once the \
point CSR would pass 2^32-1 points, because a usize count past u32::MAX would wrap to a small \
number and describe a completely different set of paths (D6) — a refusal, never a wrap or a \
truncation; a parameter outside the rules on StyleParams is refused the same way, before any \
point is written";

const PONYTAIL: &str = "none owed on the generators, which is what Phase 8 asks for and what the \
arithmetic supports: no threshold, no sampling, no fallback, no estimate, and every point is \
computed once in f64 and cast once. Two choices are conventions rather than computations and \
are stated in the parent module's doc rather than hidden — the self-loop's half-radius lift \
and the L/Z corner rule — the same treatment layout/circular.rs gives its ring spacing. \
Ponytail (loop ceiling): self_loop_segments is refused past 32, the reference's edge_segments \
ceiling (SciGraphs/properties/edge_style_properties.py:78-85), so a smoother loop is refused and \
never drawn coarser. Ponytail (scale_ceiling): an estimate — 473 B per node is derived from the topology layer's \
measured 442 and a 31 B per edge style, then projected onto wasm32's 4 GiB; re-measure with \
crates/graph-core/tests/memory.rs";

/// One registered style: its capability id and the metadata the ledger publishes for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StyleCapability {
    /// Its capability id, which is also its hash-gate stage.
    pub id: &'static str,
    /// Its ledger metadata.
    pub meta: Metadata,
}

/// Every registered style, in the order the ledger lists them.
pub static STYLES: [StyleCapability; 4] = [
    StyleCapability {
        id: Style::Straight.id(),
        meta: meta(EdgeGeometryKind::Line),
    },
    StyleCapability {
        id: Style::Orthogonal.id(),
        meta: meta(EdgeGeometryKind::Polyline),
    },
    StyleCapability {
        id: Style::Bezier.id(),
        meta: meta(EdgeGeometryKind::Curve),
    },
    StyleCapability {
        id: Style::Quadratic.id(),
        meta: meta(EdgeGeometryKind::Curve),
    },
];

/// The style registered under `id`.
pub fn find(id: &str) -> Option<&'static StyleCapability> {
    STYLES.iter().find(|capability| capability.id == id)
}

/// A style's ledger metadata. Everything but the edge kind is one answer for all four: a
/// style emits no node geometry of its own — it reads the layout's `x`/`y` and re-emits
/// them unchanged, which is what makes it compose with every layout — so `nodes` names
/// the `Point` centres they all share and only `edges` is per-style.
const fn meta(edges: EdgeGeometryKind) -> Metadata {
    Metadata {
        tier: 1,
        stage: "post",
        nodes: NodeGeometryKind::Point,
        edges,
        oracle: ORACLE,
        complexity: COMPLEXITY,
        scale_ceiling: POST_STYLE_CEILING,
        degradation: DEGRADES,
        ponytail: PONYTAIL,
    }
}

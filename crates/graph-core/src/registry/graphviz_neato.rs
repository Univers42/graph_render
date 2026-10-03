//! Ledger metadata for `layout.force.neato`, Graphviz's stress-majorization engine.
//!
//! Its own file rather than an entry in `registry/force.rs`, so the parallel Graphviz
//! engine jobs touch `registry.rs` with a one-line additive `mod` and a `LAYOUTS` append
//! and nothing else.
//!
//! **It is a force row and it is gated against a different oracle.** Every other
//! `layout.force.*` row is compared against networkx or d3-force; this one is compared
//! against Graphviz's own `neato`, through the docker-only `ge-graphviz-oracle` image
//! (`docs/decisions/graphviz-oracle.md`), because the user decided the native Graphviz
//! engines must match Graphviz's output rather than a re-derivation of the same formula
//! (2026-09-30). The `complexity` and `degradation` fields below say the same things their
//! neighbours say, because the *algorithm's* costs are the same shape whatever the oracle;
//! only the `oracle` field and the two Ponytails differ.

use super::Metadata;
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Node count `layout.force.neato` was run at, the largest size measured here.
///
/// **Measured**, `--release`, `--repeat 3` medians on one host, with
/// `scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.force.neato
/// --n 220,1000,2000,4000,10000 --past-ceiling --repeat 3`: 112 ms at 220 nodes, 2.65 s at
/// 1 000, 7.49 s at 2 000, 35.5 s at 4 000 and 251 s at 10 000 (15 474 edges), the
/// stress-1 flat across all five (0.345 to 0.382). The full table, with the per-size
/// iteration behaviour that explains the shape of it, is in
/// `docs/measurements/p13-gv2-neato.md`.
///
/// Two ceilings are in play and the lower one is the honest one. This engine is `O(n^2)` in
/// space — the packed all-pairs distance triangle is `n(n+1)/2` entries — and `O(k n^2)` in
/// time for the `k` stress passes it takes, so 100 000 nodes would want 5e9 triangle entries
/// and 20 GB at 4 bytes each. The figure above is where it was **run**, not where it was
/// found to stop working; the constraint that actually binds is the packed allocation rather
/// than the arithmetic, and at 10 000 nodes the allocation is 200 MB against 251 s of work.
pub const NEATO_CEILING: u64 = 10_000;

const DEGRADATION: &str = "past 13 376 nodes the three O(n^2) packed f32 triangles would pass \
graph_core::budget's 1 GiB and the layout refuses with StageError::Param { name: \"nodes\" } \
before allocating, on wasm32 and natively alike — a refusal, never a trap, a wrap or a \
truncation. Nothing degrades *within* the ceiling: the layout has no \
timeout and no cut-off, so a larger graph costs quadratically more time and returns the same \
kind of answer. The cost is quadratic in the *passes* as well as in the set-up, and the pass \
count is not bounded by anything small: the default 200-iteration budget is reached in full on \
the larger models, where a 3-path needs 132 and seed 44's 46-node graph 182. So the time here is \
set by convergence and the budget is a real ceiling at scale, not a formality";

pub(super) const NEATO: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Line,
    oracle: "Graphviz 16.1.0 neato -Tplain -Gstart=1 in the docker-only ge-graphviz-oracle image \
(pinned by sha256 in scripts/orch/fetch-refs.sh), over the same 1000 seeded fixtures, compared \
by harness/oracle-graphviz.py --compare neato: the largest absolute node-coordinate difference in \
points after both arms are rescaled to the same bounding box, measured at 6.73e-2 over the sweep \
(ceiling 1e-1). **-Gstart is NOT inert for this engine**, and that is the difference from \
layout.twopi: the reference seeds a drand48 generator from the start attribute and reads two \
draws per node into its initial placement (stress.c:154-155, neatoinit.c:989), so the drawing \
is a function of the seed. Measured: all 1000 fixtures draw differently at -Gstart 1, 7 and 99 \
(combined sha256 c5a0d33d / 456a4951 / 96bc5348), so this port reproduces the generator exactly \
rather than approximating a distribution. The gap is at the oracle's own printed resolution and \
not above it: -Tplain writes five significant digits, one of which is 0.911 points at the largest \
gate drawing, and the worst gap is 0.074 of that (correlation of the per-seed gap with the \
drawing's extent 0.71). Five of the six small cases (one node, two nodes, a 3-path, a 4-cycle \
and a 5-star) agree with neato -Tplain token for token at that precision, ours in \
crates/graph-core/src/layout/graphviz/neato/tests.rs and Graphviz's in \
docs/measurements/p13-gv2-neato.md; the 6-branch differs at the fourth significant digit, which \
is the iterative part of the engine and the reason the ceiling is a tolerance and not a claim \
of identity",
    complexity: "O(k n^2) time and O(n^2) space, k the stress passes: one breadth-first search \
per source for the packed all-pairs distance triangle, then per pass a Laplacian build and a \
matrix-vector product over that triangle plus a conjugate-gradient solve of up to n passes, each \
a matrix-vector product. Unlike the networkx-ported force rows, the space term is not \
O(n + m): the n(n+1)/2 packed triangle dominates at every size, which is what the scale_ceiling \
is two orders of magnitude below theirs for",
    scale_ceiling: NEATO_CEILING,
    degradation: DEGRADATION,
    ponytail: "Ponytail (disconnected input): the reference gives a node it never reached the \
distance closestDist + 10, where closestDist is the distance of the last node its breadth-first \
queue emptied on; with more than one component, which component drains last is decided by the \
order the queue happened to interleave them. This port uses its own queue and reproduces the \
arithmetic, not that choice. Failing input: any graph with a node no edge reaches. Direction: \
the unreachable node lands a different, still finite, distance off its component — cosmetic, and \
every reachable node is still placed by exactly the same arithmetic. Escape hatch: connect the \
graph; the differential's fixtures are connected by construction, so the two arms are only ever \
compared where they agree. Ponytail (stopped, not finished): Epsilon is a relative change in the \
stress and the conjugate gradient's own 1e-3 bounds a residual, so the drawing is within a \
tolerance of the stress optimum and not at it. Failing input: any graph, at the far end of its \
convergence. Direction: spacing ratios are off by the tolerance rather than exact — a 3-path's \
ends measure 1.9830 where the hop counts say 2.0, and its two adjacent pairs agree to 2e-4. \
Escape hatch: the differential, which compares this port with neato -Tplain itself and reports \
the gap the stopping rule leaves. Ponytail (rotation, not translation): the stress is invariant \
under any similarity, and the initial placement is random, so this layout has no canonical \
orientation — two callers laying out the same graph get drawings related by a rotation. The \
differential's bounding-box rescale removes the translation but NOT the rotation, which is why \
its metric is a gap on a shared box and not a set-to-set distance. Failing input: any graph \
compared at two seeds. Direction: a drawing is rotated, not merely moved. Escape hatch: the \
seed, which both arms take from -Gstart=1. Ponytail (scale_ceiling): a measured lower bound, \
and the binding constraint is the O(n^2) allocation rather than the time — see NEATO_CEILING",
};

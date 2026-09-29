//! Phase 8's POST rows: `post.route.grid`, obstacle-avoiding routing over a uniform grid
//! of the node geometry.
//!
//! Split out of `capabilities.rs` for the same reason Phase 7's `analysis.rs` is: the
//! 300-line house limit, and a POST row's metadata is long and specific in a way a
//! topology row's is not.
//!
//! `Status::Implemented`, honestly not `gated`: the routing is wired into no hash-gate
//! stage and no oracle differential — both live in `graph-cli` and `graph-wasm`, outside
//! this slice's envelope — so there is no evidence to back a `gated` claim, and
//! `problems()` only demands evidence from a `gated` row (`prompt.md` §8). The wiring is
//! the merge step's, exactly as it was for Phase 7's analysis rows
//! (`docs/measurements/phase07-analysis.md`).

use super::{Capability, Status};

/// `(id, oracle, complexity, scale_ceiling, degradation, ponytail)`.
type Row = (
    &'static str,
    &'static str,
    &'static str,
    u64,
    &'static str,
    &'static str,
);

/// The routing grid's own ceiling, and why it is this one.
///
/// Measured, natively, on the synthetic models the hash gate uses
/// (`docs/measurements/phase08-routing.md`): routing is **O(m · cells · log cells)**, one
/// Dijkstra per edge over the whole grid, so the cost is driven by the *edge* count and
/// the grid area rather than by the node count alone. At the default resolution of 128 a
/// graph of 5 000 nodes occupies a 128 × 128 grid (16 384 cells) and 7 721 edges; that is
/// the largest input measured inside a 10-second budget. This is a **time** ceiling, not a
/// memory one: the grid index and the CSR are a few hundred kilobytes at that size, and
/// nothing here refuses to run past the ceiling — it just stops being interactive.
const ROUTE_CEILING: u64 = 5_000;

const ROUTE_DEGRADES: &str = "past the ceiling routing still computes the same routes, \
exactly and in the same order — it is slower, never different, and never refuses. There is no \
built-in cutoff and no silent degradation: a caller who needs a bound applies its own \
timeout. The resolution parameter is the lever that trades cost for quality, and lowering it \
lowers the cost quadratically (see the Ponytail)";

const ROUTE: [Row; 1] = [(
    "post.route.grid",
    "hand: SciGraphs/engine/scigraphs_engine/bundling/routed.py's grid and trace are the \
reference for the *structure* (a cubic uniform grid with node cells as obstacles, and a \
walk back that minimises dist[n] + w(n, x)), but its solver is Jacobi Bellman-Ford chosen \
for GPU gather-form reasons that do not apply at tier 1a, so the solver here is \
petgraph::algo::dijkstra over the grid's own CSR — Phase 7's, not a third implementation. \
No third-party grid router exists to be a byte-for-byte oracle, so this differs against \
unit tests that pin exact cell sequences and against the 4-way hash",
    "O(m · cells · log cells); cells is quadratic in `resolution`",
    ROUTE_CEILING,
    ROUTE_DEGRADES,
    "Ponytail (resolution): `resolution` is cells across the longer axis, so the cell count \
grows quadratically with it and a fine grid is quadratically more expensive. Failing input: a \
gap between two nodes narrower than one cell — the grid cannot represent it, so the route \
either detours the long way round or, if the node is fully enclosed, falls back to the \
straight segment. Direction: **a route that detours, or a straight line drawn through a \
node — visually wrong, and the dangerous direction**, because a line through a node reads as \
a connection that the layout does not have. Reported, not hidden: Route::straight_fallback is \
true for every edge that fell back, and Routed::fallbacks counts them, so a caller can find \
every one. Escape hatch: raise `resolution`, or lower it and accept the coarseness — both \
are the caller's parameter, and the hash pins whichever is chosen. Ponytail (trace rule): the \
tie-break is by cell index, which is a stated convention rather than a computation — among \
equal-cost routes (the normal case on a uniform grid) it picks an arbitrary one, and the \
arbitrary one is fixed. Direction: cosmetic, never wrong: every tied route has the same \
cost, so the one chosen is as short as any other; it is simply not the prettiest, and \
nothing about it is unreproducible. Escape hatch: none needed for reproducibility; a \
prettier choice would be a different stated rule, not a different answer. Ponytail \
(scale_ceiling): measured on time at the default resolution and on the gate's hardware, \
projected to a node count; it is not a memory limit and not a correctness limit",
)];

/// Every POST row, its metadata carried above and its ledger shape filled in here.
pub fn rows() -> impl Iterator<Item = Capability> {
    ROUTE.iter().map(
        |&(id, oracle, complexity, scale_ceiling, degradation, ponytail)| Capability {
            id,
            tier: 1,
            stage: "post",
            geometry: Some("polyline"),
            status: Status::Implemented,
            oracle,
            oracle_record: "roundtrip",
            functions: &[],
            hash_stage: "post",
            oracle_diff: String::new(),
            hash_4way: String::new(),
            scale_ceiling,
            degradation,
            ponytail,
            complexity,
        },
    )
}

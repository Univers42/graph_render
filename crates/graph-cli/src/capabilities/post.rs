//! The POST rows: `post.route.grid` (obstacle-avoiding routing over a uniform grid of the
//! node geometry), the two bundling rows `post.bundle.fdeb` and `post.bundle.mingle`, the
//! node-overlap row `post.separate.grid`, and the four `post.style.*` rows.
//!
//! Split out of `capabilities.rs` for the same reason Phase 7's `analysis.rs` is: the
//! 300-line house limit, and a POST row's metadata is long and specific in a way a
//! topology row's is not.
//!
//! `Status::Implemented`, honestly not `gated`: no POST stage is wired into a hash-gate
//! stage and no oracle differential covers one — both live in `graph-cli` and
//! `graph-wasm`, outside this slice's envelope — so there is no evidence to back a
//! `gated` claim, and `problems()` only demands evidence from a `gated` row
//! (`prompt.md` §8). The wiring is the merge step's, exactly as it was for Phase 7's
//! analysis rows (`docs/measurements/phase07-analysis.md`).
//!
//! Only the routing row's metadata is written out here. The other seven are projected from
//! the `Metadata` their own `graph-core` module already declares
//! (`post::fdeb::META`, `post::mingle::META`, `post::separate::META`,
//! `post::styles::STYLES`), which is where the measured ceilings and the Ponytails live.
//! Restating them here would be a second answer to the same question, free to drift from the
//! code that carries them.

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
/// graph of 5 000 nodes and 7 721 edges routes on 132 × 132 cells (17 424, the margin
/// included) in 15 566.2 ms (phase 8) and 16 971.653 ms (`fix-post-routed.md`, U8): the
/// largest input measured inside a 20-second budget. 2 000 nodes take 5.9 s. This is a
/// **time** ceiling, not a memory one: the grid index and the CSR are a few hundred
/// kilobytes at that size, and no node count makes the pass refuse — it just stops being
/// interactive.
const ROUTE_CEILING: u64 = 5_000;

const ROUTE_DEGRADES: &str = "past the ceiling routing still computes the same routes, \
exactly and in the same order — it is slower, never different, and the node count never makes \
it refuse. Its one refusal is a parameter refusal, independent of the node count: a \
`resolution` and `margin` whose grid would pass u32::MAX / 8 cells is an Err(Param) \
(post/grid_index/build.rs). There is no built-in cutoff and no silent degradation: a caller \
who needs a bound applies its own timeout. The resolution parameter is the lever that trades \
cost for quality, and lowering it lowers the cost quadratically (see the Ponytail)";

const ROUTE: [Row; 1] = [(
    "post.route.grid",
    "hand: SciGraphs/engine/scigraphs_engine/bundling/routed.py's grid and trace are the \
reference for the *structure* (a cubic uniform grid and a walk back that minimises \
dist[n] + w(n, x)), with three stated divergences (post/routed.rs, post/grid_index.rs). \
Another node's cells are impassable here; the reference prices them at a finite \
1 + avoid · gain · density with avoid 0 by default, so at default settings it routes \
straight through a node this pass detours around (R8). The grid is sized on the node \
footprints, radius or half-size included, where the reference sizes it on the coordinates \
alone, so only a point layout is comparable cell for cell (M22). A resolution below 8 is \
used as given, where the reference raises it to 8 (U18). The reference's solver is Jacobi Bellman-Ford chosen \
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
    ROUTE
        .iter()
        .map(
            |&(id, oracle, complexity, scale_ceiling, degradation, ponytail)| {
                let meta = RowMeta {
                    oracle,
                    complexity,
                    scale_ceiling,
                    degradation,
                    ponytail,
                };
                row(id, "polyline", meta)
            },
        )
        .chain(bundles())
        .chain(styles())
}

/// The rows projected from the metadata their own `graph-core` modules declare:
/// [`graph_core::post::fdeb::META`], [`graph_core::post::mingle::META`] and
/// [`graph_core::post::separate::META`].
///
/// Read across rather than restated: a second, looser copy of a ceiling or a Ponytail is a
/// second answer, and the two would drift. None is `gated` — see the module doc.
///
/// **The id and the `META` of one module are declared together**, each taken from that
/// module's own `ID`. They used to be `zip`ped against a list beside them, so reordering
/// either list would silently attach one module's ceiling, oracle and Ponytail to another's
/// stable row id, with no compile error anywhere. `separate`'s `edges` is `Line` because the
/// pass is **pass-through** — it emits the edges it was handed — and there is no
/// `EdgeGeometryKind` for "the same edges"; its `Metadata::moves_nodes` flag is what tells a
/// reader to read that column that way.
fn bundles() -> impl Iterator<Item = Capability> {
    use graph_core::post::{fdeb, mingle, separate};
    [
        (fdeb::ID, fdeb::META),
        (mingle::ID, mingle::META),
        (separate::ID, separate::META),
    ]
    .into_iter()
    .map(|(id, meta)| row(id, edge_kind_name(meta.edges), RowMeta::of_post(&meta)))
}

/// The four style rows, projected from [`graph_core::post::styles::STYLES`]. They differ
/// in exactly one field, `edges`: a style never emits two geometry kinds, so each row
/// names the one its generator emits, and `EdgeGeometry::Line` (the straight chord) is
/// named as it is stored rather than folded into `polyline`.
fn styles() -> impl Iterator<Item = Capability> {
    graph_core::post::styles::STYLES.iter().map(|style| {
        row(
            style.id,
            edge_kind_name(style.meta.edges),
            RowMeta::of_registry(&style.meta),
        )
    })
}

/// The ledger's name for an edge geometry kind, as `registry::layout` writes it.
///
/// Exhaustive on purpose, with no catch-all: it used to answer `"Curve"` for anything it
/// did not recognise, so a fourth kind added to the enum would have landed in the ledger
/// as a confidently wrong geometry. A new kind is now a compile error here instead.
fn edge_kind_name(kind: graph_contract::geometry::EdgeGeometryKind) -> &'static str {
    use graph_contract::geometry::EdgeGeometryKind as K;
    match kind {
        K::Line => "Line",
        K::Polyline => "Polyline",
        K::Curve => "Curve",
    }
}

/// The five metadata fields every POST row carries, grouped so `row` stays at three
/// parameters.
struct RowMeta {
    oracle: &'static str,
    complexity: &'static str,
    scale_ceiling: u64,
    degradation: &'static str,
    ponytail: &'static str,
}

impl RowMeta {
    fn of_post(meta: &graph_core::post::Metadata) -> Self {
        Self {
            oracle: meta.oracle,
            complexity: meta.complexity,
            scale_ceiling: meta.scale_ceiling,
            degradation: meta.degradation,
            ponytail: meta.ponytail,
        }
    }

    fn of_registry(meta: &graph_core::registry::Metadata) -> Self {
        Self {
            oracle: meta.oracle,
            complexity: meta.complexity,
            scale_ceiling: meta.scale_ceiling,
            degradation: meta.degradation,
            ponytail: meta.ponytail,
        }
    }
}

/// One POST row's ledger shape. `oracle_record` is `roundtrip` and `functions` is empty
/// for every row here: no POST stage is in the hash gate's list and no differential
/// covers one, so a `gated` claim would be one `problems()` has to refuse. `hash_stage` is
/// the row's own id, so adding a stage to the gate later is a one-word change here rather
/// than a silent mismatch.
fn row(id: &'static str, geometry: &'static str, meta: RowMeta) -> Capability {
    Capability {
        id,
        tier: 1,
        stage: "post",
        geometry: Some(geometry),
        status: Status::Implemented,
        oracle: meta.oracle,
        oracle_record: "roundtrip",
        functions: &[],
        hash_stage: id,
        oracle_diff: String::new(),
        hash_4way: String::new(),
        scale_ceiling: meta.scale_ceiling,
        degradation: meta.degradation,
        ponytail: meta.ponytail,
        complexity: meta.complexity,
    }
}

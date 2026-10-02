//! Phase 7's ANALYSIS rows: pure functions over the existing topology (`crate::analysis`
//! in graph-core), reused via petgraph where petgraph has the algorithm and hand-written
//! (Brandes, Wasserman & Faust, power iteration, networkx's Louvain gain formula) where
//! it does not. None of these are wired into the 4-way hashgate or the TS oracle
//! differential yet — `hashgate.rs` and `graph-wasm` are outside this phase's
//! authorization envelope, so that wiring, and exposing the results in the snapshot,
//! JSON and SDK, is deferred to the merge step (`docs/measurements/phase07-analysis.md`).
//! Every row below is therefore `Status::Implemented`, honestly not `gated`: `problems()`
//! only demands hash/oracle evidence from a `gated` row, so an `Implemented` one never
//! reports a claim its evidence cannot back (`prompt.md` §8). `analysis.depth` is
//! registered too: it was held back only because p3's `layout/hierarchy.rs` was not on
//! the base its phase was written on, and `impl depth::Roots for Hierarchy` is now in
//! `graph-core` (`analysis/depth.rs`), so the row's oracle is a call that exists rather
//! than a sentence. See `docs/reports/phase-07.md` §2 and §7 for that row's history.
//!
//! Split out of `capabilities.rs` itself only to keep that file under the house's
//! 300-line limit — same reason `registry.rs` and `verdict.rs` are already separate
//! submodules of it.

use super::registry::TOPOLOGY_CEILING;
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

/// Memory-bound rows carry the same per-node cost class as the topology columns they
/// read (`TOPOLOGY_CEILING`'s own measurement): one or two more `Vec<u32>` or `Vec<f32>`
/// columns is not a new order of magnitude. The algorithm itself is exact — nothing
/// owed there — but the *ceiling* is an inherited, not independently re-measured,
/// estimate, which is what earns every row below its own Ponytail marker.
const MEMORY_DEGRADES: &str = "past the ceiling, the same shape as topology: wasm32 cannot \
allocate and the module traps (no partial result); natively, memory permitting, this refuses \
alongside index_model's own CapacityError rather than wrapping or truncating";

const MEMORY_PONYTAIL: &str = "none owed on the algorithm itself (exact). Ponytail \
(scale_ceiling): inherited from topology's measured per-node cost, not independently \
re-measured for this column's own shape; re-measure with crates/graph-core/tests/memory.rs";

const ROWS: [Row; 10] = [
    (
        "analysis.components.weak",
        "hand: union-find; no TS oracle exists, so this differs against unit tests and the \
4-way hash rather than a differential",
        "O(n + m)",
        TOPOLOGY_CEILING,
        MEMORY_DEGRADES,
        MEMORY_PONYTAIL,
    ),
    (
        "analysis.components.strong",
        "hand: petgraph::algo::tarjan_scc; no TS oracle exists, so this differs against \
unit tests and the 4-way hash rather than a differential",
        "O(n + m)",
        TOPOLOGY_CEILING,
        MEMORY_DEGRADES,
        MEMORY_PONYTAIL,
    ),
    (
        "analysis.paths.dijkstra",
        "petgraph::algo::dijkstra, reused (precondition: no negative edge weight)",
        "O((n + m) log n)",
        TOPOLOGY_CEILING,
        MEMORY_DEGRADES,
        MEMORY_PONYTAIL,
    ),
    (
        "analysis.paths.bellman_ford",
        "petgraph::algo::bellman_ford / find_negative_cycle, reused — the negative-weight \
defect SciGraphs's operator silently avoids by always dispatching Dijkstra instead \
(docs/tutorials/panels/scigraphs/algorithms.qmd:74-75) is what this fixes",
        "O(n * m)",
        TOPOLOGY_CEILING,
        MEMORY_DEGRADES,
        MEMORY_PONYTAIL,
    ),
    (
        "analysis.centrality.degree",
        "topology's own degree column (Phase 1), reused verbatim, not recomputed",
        "O(1)",
        TOPOLOGY_CEILING,
        MEMORY_DEGRADES,
        MEMORY_PONYTAIL,
    ),
    (
        "analysis.centrality.closeness",
        "hand: Wasserman & Faust's disconnected-safe closeness formula, as networkx 3.6 \
computes it (algorithms/centrality/closeness.py:127-133, wf_improved): every reached node \
counts, a peer at distance 0 included. Precondition: non-negative edge weights",
        "O(n * (n + m) log n)",
        TOPOLOGY_CEILING,
        MEMORY_DEGRADES,
        MEMORY_PONYTAIL,
    ),
    (
        "analysis.centrality.betweenness",
        "hand: Brandes (2001), weighted (Dijkstra-based) — petgraph has no betweenness. \
Precondition: strictly positive edge weights (a zero weight has no finite path count; igraph \
0.11.9 refuses it too, src/centrality/betweenness.c:436-437)",
        "O(n * (n + m log n)): per source, Θ(n) fresh buffers plus a Dijkstra (this weighted, \
Dijkstra-based Brandes; costlier than the O(n*m) unweighted/BFS form the phase names, a \
recorded deviation for internal consistency with paths.rs's weighted Dijkstra/Bellman-Ford)",
        20_000,
        "past the ceiling exact betweenness is still correct, just not interactive: measured \
(docs/measurements/phase07-analysis.md) 475 ms at 16,000 nodes/24,793 edges and 3.1 s at \
32,000/49,613, real wall-clock in the gate container. No sampled variant ships under this name, \
so nothing degrades silently past it — the caller waits, or does not call it at this scale",
        "Ponytail: the ceiling is a usability judgement (interactive vs. not), never a memory or \
correctness limit — failing input: any graph past ~20k nodes on the gate's hardware; direction: \
slow, never wrong; escape hatch: a separately named sampled variant, not shipped here (no \
consented sampling error on disk to back one, prompts/phase-07-analysis.md stop-and-ask 3)",
    ),
    (
        "analysis.centrality.eigenvector",
        "hand: power iteration, fixed start vector, sign-pinned, L2-normalised — Phase 6's \
`_eig_start_vector` reasoning ported fresh (its own module is not on this branch's base)",
        "O(k * (n + m)) for k iterations to convergence or the fixed cap",
        TOPOLOGY_CEILING,
        MEMORY_DEGRADES,
        "Ponytail: may not converge on a disconnected or bipartite(-ish) graph, whose two \
extremal eigenvalues tie in magnitude (a star is the textbook case); failing input: such a \
graph; direction: plausible-looking, not obviously wrong — a caller ranking across components \
could be misled; escape hatch: the returned `bool` is true only once the residual is below \
tolerance, false (with the last iterate) otherwise — never silently claimed as converged",
    ),
    (
        "analysis.communities.louvain",
        "networkx 3.6 algorithms/community/louvain.py's modularity-gain formula, ported: \
local-moving phase only, no multi-level aggregation (a recorded scope deviation) and no RNG at \
all — dense-index order plus a strict-gain tie-break replace networkx's seeded shuffle",
        "O(passes * (n + m)); no formal bound on pass count, none observed in practice",
        TOPOLOGY_CEILING,
        MEMORY_DEGRADES,
        "Ponytail: a heuristic, order-dependent result; failing input: a graph with near-tied \
modularity gains between two candidate moves; direction: cosmetic, never dangerous — returns a \
valid partition, just not necessarily the highest-modularity one; escape hatch: none needed for \
reproducibility (no seed to vary); a different fixed visit order is a different, equally valid \
answer",
    ),
    (
        "analysis.depth",
        "Phase 3's repaired layout/hierarchy.rs — one Hierarchy, one root set, one depth. \
analysis/depth.rs derives no roots, cycles or parents of its own: it declares a four-method \
`Roots` trait and p3's `Hierarchy` implements it by delegation (`impl depth::Roots for \
Hierarchy`), so the two are one convention with two names. It does check that convention \
(an out-of-range root, or two roots with no virtual root, panics in every build) and reads \
the root offset with p3's own `roots.len() >= 2` test. This row differs against Phase 3's own \
`Hierarchy::depth` column node by node over its four fixtures rather than against a \
TypeScript oracle — none exists for breadth-first depth over a repaired tree",
        "O(n + m): one breadth-first pass, each node's children row read once",
        TOPOLOGY_CEILING,
        MEMORY_DEGRADES,
        "none owed on the walk (exact: no threshold, no sampling, no fallback, no estimate). \
Ponytail (UNREACHED): a node no declared root reaches reads `u32::MAX`, deliberately not 0 — \
Phase 3's own column leaves an unreachable node at 0, but that column is only ever read on a \
repaired tree where nothing is unreachable, while this one is an analysis result a frontend \
may colour by and 0 would render an orphan as a root. Failing input: a forest whose roots \
were declared by hand rather than repaired, i.e. `depth_from` over a partial tree. Direction: \
not refused: `Depth::levels` and the wasm JSON face hand the raw `u32::MAX` out, so a consumer \
that uses a level as a ring index without testing `Depth::is_unreached` (or comparing with \
`UNREACHED`) places the node at ring 4294967295. Escape hatch: `Depth::is_unreached`, or \
`bfs_depth` over a repaired `Hierarchy`, \
where every node is reached — pinned by graph-core \
analysis/depth/hierarchy.rs::a_repaired_hierarchy_reaches_every_node_so_unreached_never_appears. \
Ponytail (scale_ceiling): inherited from topology's measured per-node cost, not independently \
re-measured for this column's own shape (one `u32` per node plus a queue of the same order); \
re-measure with crates/graph-core/tests/memory.rs",
    ),
];

/// Every analysis row, its metadata carried above and its ledger shape filled in here.
pub fn rows() -> impl Iterator<Item = Capability> {
    ROWS.iter().map(
        |&(id, oracle, complexity, scale_ceiling, degradation, ponytail)| Capability {
            id,
            tier: 1,
            stage: "analysis",
            geometry: None,
            status: Status::Implemented,
            oracle,
            oracle_record: "oracle-diff",
            functions: &[],
            hash_stage: "analysis",
            oracle_diff: String::new(),
            hash_4way: String::new(),
            scale_ceiling,
            degradation,
            ponytail,
            complexity,
        },
    )
}

> **Status (2026-10-02):** MERGED into develop, merge ffb837a (branch p7 head ab396f5). Closed: docs/reports/phase-07.md landed on develop after f261baf (commit 3ddd308). See docs/reports/STATUS.md.

# Phase 7 — The ANALYSIS stage

**Read `prompt.md` and `prompts/REFERENCES.md` first.** Phase 6's gate must be green.

## Goal

Graph analysis as a first-class pipeline stage: communities, centrality, shortest paths, components,
hierarchy depth. Results become attribute columns that later stages (and any frontend) can consume.

## Why it is a stage, not a utility

The user's driving complaint is that *"iterating the analysis of graph in real time with python is
completely crazy."* Analysis is the thing that was too slow — so it is the thing that most needs to be in
Rust, and it needs to be **composable with layout**, not bolted beside it. A community assignment feeds
force-layout clustering; a centrality score feeds node size; a BFS depth feeds radial placement. Those are
stage-to-stage dependencies, which is exactly what the pipeline exists to express.

## Reuse before implement — this phase is mostly wiring

`petgraph` already implements Dijkstra, Bellman-Ford, A*, connected components, topological sort and MST.
**Implement petgraph's traits on our CSR** (`IntoNeighbors`, `IntoNodeIdentifiers`, `Visitable`,
`IntoEdgeReferences`) and reuse its algorithms. One topology representation, no rewritten textbook code.

This is the one phase that opens `graph-core`'s dependency allow-list, and it is pre-authorised for
`petgraph` only. Anything else is a stop-and-ask.

**Determinism audit is part of the work, not an afterthought.** For every petgraph algorithm used, verify
that no `HashMap` iteration order reaches the output. Where it does, wrap it: collect, sort by dense index,
then emit. An algorithm is not `gated` until this audit is recorded.

## The defect we fix rather than inherit

SciGraphs offers **Bellman-Ford** in its pathfinding UI, but the operator always dispatches Dijkstra
instead, silently (`docs/tutorials/panels/scigraphs/algorithms.qmd:74-75`). So negative-weight shortest
paths do not work, and nothing says so. We implement it, and we **test the negative-weight case
specifically** — that test is the phase's proof that the capability claim is real. It is also the concrete
instance of guardrail 8.

## Authorization envelope

**CREATE — exactly these:**
```
crates/graph-core/src/analysis/{mod.rs,components.rs,paths.rs,centrality.rs,communities.rs,depth.rs}
crates/graph-core/src/csr_petgraph.rs         (trait impls only — no algorithms)
fixtures/analysis/{weighted.json,negative-weight.json,disconnected.json,star.json,two-cliques.json}
docs/decisions/petgraph-determinism-audit.md  (REQUIRED)
docs/measurements/phase07-analysis.md
```

**MODIFY — exactly these:**
```
crates/graph-core/src/lib.rs
crates/graph-core/src/registry.rs
crates/graph-core/Cargo.toml                  (add petgraph — pre-authorised, pinned)
crates/graph-cli/src/{capabilities.rs,main.rs}
```

**FORBIDDEN:** `src/`, `tests/`, `verify/`. Any layout. Edge routing (Phase 8). Anything in osionos.
`pysurprise`/SurpriseMe is out of scope (a native binding, `prompt.md` §10).

## Steps

### 1. `csr_petgraph.rs` — trait impls, nothing else

Implement petgraph's graph traits over our existing CSR. **No data is copied into a petgraph container.**
If you find yourself building a `petgraph::Graph`, stop — that is a second topology representation, and it
will drift from the first.

Neighbour iteration must preserve our CSR order (Phase 1 matched the oracle's Map order deliberately).

### 2. `components.rs`

Connected (and weakly/strongly connected for directed input). Component ids assigned in **ascending
dense-index-of-first-member order** — not discovery order, which depends on traversal start.

### 3. `paths.rs` — Dijkstra **and** a real Bellman-Ford

- Dijkstra for non-negative weights. Tie-break equal distances by dense index (a binary heap does not
  guarantee order among equals → D5).
- **Bellman-Ford** for graphs with negative weights, including **negative-cycle detection** that reports
  the cycle rather than returning nonsense.
- `fixtures/analysis/negative-weight.json` and a test asserting Bellman-Ford returns a *different and
  correct* answer where Dijkstra is wrong. This test is the point of the phase.

### 4. `centrality.rs`

Degree (already in the topology columns — reuse, do not recompute), closeness, betweenness (**Brandes'
algorithm** — O(nm), the only tractable exact method), eigenvector centrality (power iteration; fixed
start vector and sign pinning, exactly as Phase 6's eigen work — reuse `_eig_start_vector`'s reasoning
rather than reinventing it).

Betweenness at 100k nodes is O(nm) and will not finish. Declare the ceiling and offer sampled betweenness
as a **separately named capability** — never silently sample under an exact name. That is precisely the
Bellman-Ford defect in a new costume.

### 5. `communities.rs`

Louvain (or Leiden). Reference `SciGraphs/engine/scigraphs_engine/communities.py`.

Louvain is **stochastic and order-dependent**. Determinism requires: a fixed node visit order (dense
index), a deterministic tie-break on equal modularity gain, and a seeded RNG threaded explicitly from
`rng.rs` (Phase 6) — **no ambient RNG**. Record the modularity achieved in the measurements file; it is the
quality claim.

### 6. `depth.rs`

BFS depth from declared or detected roots, reusing Phase 3's `hierarchy.rs` root/forest logic. One
convention across the codebase, not two.

### 7. Analysis results are attribute columns

Each result is a SoA column on the topology (`community: Vec<u32>`, `betweenness: Vec<f32>`, …), hashed as
part of the analysis stage, and exposed through the SDK and the JSON snapshot. That is what makes them
usable by a frontend that wants to size or colour by centrality.

## Gate

```sh
docker run --rm -v "$PWD:/w" ge-rust cargo fmt --check                                        # 0
docker run --rm -v "$PWD:/w" ge-rust cargo clippy --workspace -- -D warnings                   # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test --workspace                                    # 0
docker run --rm -v "$PWD:/w" ge-rust cargo build -p graph-core --target wasm32-unknown-unknown  # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- hashgate --seeds 1000            # 0
docker run --rm -v "$PWD:/w" -e GM_MUTATE_REFERENCE_DEGREE=9 ge-rust \
  cargo run -p graph-cli -- hashgate --seeds 8                                                 # NON-ZERO

# the defect we refused to inherit
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core bellman_ford_negative_weight      # 0
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core negative_cycle_detected           # 0

# determinism: repeated runs agree, and no HashMap order leaks
docker run --rm -v "$PWD:/w" ge-rust cargo test -p graph-core analysis_determinism              # 0

# only petgraph was added
# (`graph-contract` is in the regex because it is graph-core's own workspace path
#  dependency, present since Phase 0 — without it this row fails on the pristine base)
docker run --rm -v "$PWD:/w" ge-rust sh -c \
  'cargo tree -p graph-core --depth 1 | tail -n +2 | grep -vE "libm|indexmap|petgraph|graph-contract" | grep . && exit 1 || exit 0'  # 0

docker run --rm -v "$PWD:/w" ge-rust cargo run -p graph-cli -- capabilities --check             # 0
docker run --rm -v "$PWD:/w" -w /w node:22-slim node harness/sdk-smoke.mjs                      # 0
docker build -t ge-check . && docker run --rm ge-check                                          # 0
```

## Ledger delta

To `gated`: `analysis.components`, `analysis.paths.dijkstra`, `analysis.paths.bellman_ford`,
`analysis.centrality.{degree,closeness,betweenness}`, `analysis.communities.louvain`, `analysis.depth`.

`analysis.centrality.betweenness` must declare its O(nm) ceiling. If sampled betweenness ships, it is a
**separate id** with its own `ponytail` naming the sampling error.

## Ponytail requirements

- **Louvain is a heuristic** and order-dependent: name the failing input (a graph with near-tied modularity
  gains), the direction (a valid but non-optimal partition — cosmetic), the escape hatch (the seed).
- **Sampled betweenness**, if shipped: name the sampling error and the direction (**under-estimates
  high-centrality nodes — dangerous if used for filtering**).
- **Eigenvector centrality** power iteration: may not converge on disconnected or bipartite-ish graphs;
  name what is returned when it does not.
- **No marker** on Dijkstra, Bellman-Ford or components — they are exact.

## Stop-and-ask

- A petgraph algorithm's output depends on `HashMap` order and cannot be wrapped deterministically → stop.
  Do not ship it; write it ourselves or defer.
- Adding a dependency beyond `petgraph` → stop. The allow-list is closed and this phase opened it exactly
  once.
- Betweenness is needed at a scale where exact is intractable → stop and ask whether sampled-under-a-
  separate-name is acceptable. Do not silently sample.

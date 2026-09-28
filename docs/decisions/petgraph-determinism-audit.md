# petgraph determinism audit — every algorithm this phase reuses

Status: **decided** (Phase 7). Scope: every `petgraph::algo::*` call from
`crates/graph-core/src/analysis/*.rs`, over `csr_petgraph::CsrDigraph`.

## Why this file exists

`prompts/phase-07-analysis.md`: *"Determinism audit is part of the work, not an
afterthought. For every petgraph algorithm used, verify that no `HashMap` iteration
order reaches the output. Where it does, wrap it: collect, sort by dense index, then
emit. An algorithm is not `gated` until this audit is recorded."* None of this phase's
rows are `gated` yet regardless (`docs/measurements/phase07-analysis.md`: the 4-way
hashgate and oracle-diff wiring is deferred to the merge step), but the audit is the
same work either way and is not deferred with them.

`CsrDigraph`'s own traversal order is not itself at risk: `IntoNeighbors`/`IntoEdges`
walk `Vec<u32>` CSR rows (`csr_petgraph.rs`), and `IntoNodeIdentifiers` walks
`0..node_count()` — both fixed, dense-index order, no hashing anywhere in this crate's
own trait impls. The question below is only ever about what a **called** petgraph
algorithm does internally.

## `petgraph::unionfind::UnionFind<u32>` (`components::weak`)

Backed by two `Vec<u32>` (`parent`, `rank`), indexed by dense id. `find`/`union` never
allocate a `HashMap` and never iterate one. **No audit needed — it was never at risk.**
`weak` still asserts (`debug_assert_eq!`) that `petgraph::algo::connected_components`
(same crate, same trait impls, itself `Vec`-based per below) agrees on the component
*count* — a cross-check against a second implementation, not a determinism requirement
in itself.

## `petgraph::algo::tarjan_scc` (`components::strong`)

petgraph 0.8.3's Tarjan is iterative, over `Vec`s indexed by `NodeIndexable::to_index`
(a stack, an index/lowlink `Vec`, an on-stack `Vec<bool>`) and pushes onto a `Vec<Vec<G::
NodeId>>` result in the order components finish — a function of visit order and CSR row
order alone, both already fixed. **No `HashMap` anywhere in the algorithm. Clean.** The
result vector's *component order* is discovery order (not dense-index-of-first-member),
which is why `components.rs` still canonicalises it the same way as `weak` — a
usability choice, not a determinism fix.

## `petgraph::algo::dijkstra` (`paths::dijkstra_distances`)

**This is the one real finding.** petgraph 0.8.3's `dijkstra` returns
`HashMap<G::NodeId, K, S>` (`hashbrown`, the default `S`), and *insertion* order into
that map depends on finalization order during the search — a real, if usually
low-consequence, order dependency for anything that iterates the map directly.

`dijkstra_distances` never iterates it: it probes the map once per dense index,
`0..node_count()`, in order —

```rust
(0..topology.node_count())
    .map(|v| scores.get(&NodeIx(v)).copied().unwrap_or(f64::INFINITY))
    .collect()
```

— so the returned `Vec<f64>` is dense-index order regardless of the map's internal
layout. **Wrapped, per the phase's own instruction, by probing rather than iterating.**

A second, narrower question: could the *distances themselves* (not their order) depend
on how the binary heap inside `dijkstra` breaks ties between equal-cost frontier
entries? No — a shortest-path *distance* is a property of the graph, not of visit order;
two entries tied at the same cost cannot produce two different correct distances for the
same node. (A *predecessor* or *path* pick could differ under a tie; this function
returns distances only, so that risk does not apply here. `centrality::betweenness`'s
own hand-rolled Dijkstra, below, is the one place this project computes predecessors,
and it is not petgraph's.)

## `petgraph::algo::bellman_ford` / `find_negative_cycle` (`paths::bellman_ford`)

Both operate over plain `Vec<K>`/`Vec<Option<NodeId>>`, indexed by
`NodeIndexable::to_index`, relaxed in `edge_references()`'s fixed order for `n-1`
rounds, then one more round to detect an improvement. `find_negative_cycle` walks
predecessor `Vec`s back from the last node relaxed in that final round — deterministic
given a deterministic `edge_references()` (ours is: source-then-target, columns order,
`csr_petgraph.rs`). **No `HashMap`. Clean.**

## Hand-written code that is not petgraph, for completeness

Not in scope for this audit (nothing here calls into petgraph), listed so "every
algorithm" in the phase text is answered in one place:

- `centrality::betweenness`'s own Dijkstra-with-path-counting (`shortest_path_dag`) uses
  a local `BinaryHeap<MinScore>` and `Vec`s throughout — `MinScore`'s `Ord` breaks ties
  by the smaller dense index (D5), so even the heap's pop order is fixed, not merely the
  final scores.
- `centrality::eigenvector` — plain `Vec<f64>` accumulation over `CsrDigraph`'s own
  fixed, dense-index-order edge iteration (the same iteration this audit already clears
  above), summed node `0..n` ascending each power-iteration step. No `HashMap`, no
  petgraph algorithm call. **No determinism defect** — listed here only so "every
  algorithm this phase reuses" (this file's own stated scope) is answered for it too, not
  because power iteration itself was ever in question.
- `communities::louvain` visits nodes `0..n` and each node's neighbour-community weights
  sorted by community id (`neighbor_weights`) — no hashing, a `Vec<(u32, f64)>` linear
  scan (small per-node degree; a `HashMap` would be the wrong tool here even ignoring
  determinism).

## What would fail this audit

An algorithm whose *emitted result* (not an internal scratch structure) is built by
iterating a `HashMap`/`HashSet` without an intervening sort — e.g. a hypothetical
petgraph API that returned `Vec<NodeId>` collected via `map.keys()` with no sort after.
None of the four algorithms above do this. If a future revision adds one that does, the
fix is the phase's own instruction: collect, sort by dense index (or another total,
input-independent key), then emit — never ship the raw iteration order.

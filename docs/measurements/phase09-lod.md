# Phase 9 — the SCALE stage: what it decides, and what it refuses to decide

`crates/graph-core/src/scale/{lod,simplify,adaptive}.rs`. Three modules, one idea: the
motor tells a front what it could draw instead of drawing less for it.

## `scale.lod` — hints as columns, never a mutated topology

`lod::hints(&topology, &x, &y, &params)` returns a `visible: Vec<u8>` mask, a
`labelled: Vec<u8>` mask, an `edges: Vec<u8>` mask and a `Tier`. The topology is not
touched: `the_hints_never_mutate_the_topology` re-reads the node and edge column byte
lengths after a run that culls 29 of 30 nodes and finds them identical. A front may ignore
every hint and still be correct, only slower — that is what makes the module safe to be
wrong about.

The policy is the caller's (`LodParams`), and the tier ladder is the phase's own:

| nodes | tier | edges | labels |
|---:|---|---|---|
| ≤ 2 000 | `Full` | all | up to `label_budget` (64) visible nodes, by degree |
| ≤ 20 000 | `NoLabels` | 1 in 8 by edge index | none |
| ≤ 200 000 | `Decimated` | 1 in 8 by edge index | up to the budget, by degree |
| > 200 000 | `Clustered` | none | up to the budget, by degree |

What is ported from the reference (`SciGraphs/engine/scigraphs_engine/lod.py`, submodule pin
`b7ccee6`) is the *shape*: the descending `argsort` of the importance key, the cumulative
cut that keeps the longest prefix within the budget, and the mask written back through the
order (`apply_budget`, `lod.py:81-85`), plus its never-empty guarantee for a budget of at
least one (`lod.py:86-88`) — both sides read a budget of zero or less as *no limit*, not as
zero labels (`lod.py:79-80`, `lod.rs:225-228`). Inside a class of **equal** keys the two
arms are not required to agree, and the motor's rule there is its own (D2, ascending dense
index); `harness/oracle-scale.py` decides which cases those are from the degrees and the
budget alone, so the number of compared cases does not move with the host's sort. The two
tests that pin the budget are
`the_label_budget_is_never_empty_and_ranks_by_degree_then_index`
(`crates/graph-core/src/scale/lod/tests.rs:115`) and
`a_label_budget_of_zero_means_no_limit`
(`crates/graph-core/src/scale/lod/mask_tests.rs:10`). What is **not**
ported is the numbering: the reference's thresholds are on-screen pixels of a 1080-tall
image and a headless motor has no pixels, so the tier comes from the node count and the
cull test is a rectangle test on world coordinates (`Viewport`). The importance key is
degree.

**Ponytail, with its failing input and its direction.** The thresholds are a heuristic and
it fails in the dangerous direction: a graph whose important nodes are *low-degree* — a
dependency graph's entry points, a star's hub the budget happens to rank low, meaning that
lives in a handful of `degree == 1` nodes — loses exactly what a reader came for.
`a_low_degree_meaningful_node_loses_its_label_to_a_higher_degree_one` is that input,
asserted rather than described. **Direction: hiding meaningful nodes.** Escape hatch: the
hints are advisory; ignore them. The second heuristic, edge decimation, fails the same way
— a stride over edge index, so a graph whose long-range edges share one stride class loses
all of them.

Cost is `O(n + m)`, one pass each, and **no spatial structure is built**. The phase prompt
asks for Phase 8's `grid_index` here; that module is not on this branch's base (p8 is a
separate worktree), and the alternative — a second spatial index inside `lod` — is exactly
what the prompt forbids. An `O(n)` rectangle test is the honest answer until `grid_index`
lands, and the deviation is recorded in `docs/reports/phase-09-progress.md`.

## `scale.simplify` — reversible, or it is data loss

Three passes, in this order, journalled in this order:

1. **Leaf folding** — every degree-1 node folds into its neighbour; the neighbour's own
   representative is inherited, so a run of leaves folds to the lowest index.
2. **Chain contraction** — each maximal run of degree-2 nodes between two *branch* nodes
   (degree ≠ 2) contracts to one representative-level edge. A run that closes on itself is
   a bare ring: there is no second endpoint, so it is left whole rather than half-drawn.
3. **Community collapse** — every community folds onto its lowest dense index, reusing
   Phase 7's `analysis::communities::louvain`.

Every removal is a `Step { kind, representative, nodes, edges, links }`: what went, into
what, which original edges went with it, and which representative-level edges the step
*implies*. `restore(&simplified)` rebuilds the original from the journal alone — no
topology is retained inside the value for the purpose — and returns the node mask, the
edge mask and the identity representative column. The phase's gate row,
`cargo test -p graph-core simplify_reversible`, is that claim:
`simplify_reversible_restores_the_original_nodes_edges_and_representatives` runs it over
five plans, `simplify_reversible_holds_on_a_mixed_graph` over a graph with leaves, a ring,
chains and five dense cores at once.

**What simplification changes, named: a contracted chain looks like an edge.** The
interior nodes are gone and the two endpoints are adjacent in the simplified graph with no
edge between them, so a front that ignores `Step::links` draws two isolated nodes where a
path was. The reversal path is `restore`; the drill-back path is the journal, one community
at a time.

**Ponytail.** The community collapse trusts Louvain, which is a heuristic and
order-dependent. Failing input: near-tied modularity gains, or a graph whose communities
are chains of a single edge, where a collapse removes the node a reader came to see.
Direction: cosmetic, because the journal still holds it — the dangerous version, an
irreversible collapse, is not implemented. Escape hatch: `Plan::collapse_communities` off.

A later pass never resurrects what an earlier one removed: a node the leaf pass folded
stays folded even when it is its community's lowest index.

## `scale.adaptive` — a budget that is a function of size, never of the clock

`tick_budget(n, m)` is `TICK_WORK / (n·log₂(n+1) + m)` — the work one settle may spend
divided by what one tick costs at that size — quantised **down** to a power of two and
clamped to `MIN_TICKS..=MAX_TICKS`, with `MAX_TICKS = 112` the full
`alphaDecay(0.06)`-to-`alphaMin` settle.

| n | m | ticks |
|---:|---:|---:|
| 0 | 0 | 8 (the floor: nothing to lay out) |
| 220 | 327 | 112 (the full settle) |
| 10 000 | 15 497 | 32 |
| 100 000 | 154 978 | 8 (the floor) |
| 1 000 000 | 1 549 780 | 8 (the floor) |

The ladder is pinned in `the_ladder_is_pinned_at_the_phase_sizes`, and the three properties
that make it a budget rather than a guess are pinned beside it: more nodes or more edges
never earns a longer settle; the answer is always in the clamp and is always a power of two
or the maximum; and the function is pure, so calling it twice with the same arguments
cannot differ.

**This is a deliberate divergence from the reference.** `adaptive.py` adapts at *render*
time from measured crowding and a camera. A motor that adapted from a clock would stop at
a different tick on a loaded host and its output would stop being reproducible — D8 says no
wall-clock in the motor, and the phase's gate greps `crates/graph-core/src` for
`Instant::now|SystemTime|elapsed` to keep it that way. The grep is the enforcement; the
purity test is the belt.

**Ponytail.** A large graph gets fewer ticks and therefore a **less settled** layout — the
same picture with its tails still moving. Failing input: any graph past ~3 000 nodes, whose
layout is under-settled at the budget. Direction: cosmetic. Escape hatch:
`tick_budget_with`'s explicit override, honoured verbatim, including `Some(0)`.

## What this stage does not do

- It does not measure anything. `scale.adaptive`'s constants are a policy, and the
  measurement campaign lives in `phase09-bench.md` and `phase09-crossover.md`.
- It does not build a spatial index, and it does not use Phase 8's `grid_index` because
  that module is not on this branch.
- It is not in the hash gate, so its three ledger rows are `implemented`, never `gated`
  (`crates/graph-cli/src/capabilities.rs`'s `scale_rows`). `scale.lod` and `scale.simplify`
  are pinned by the `oracle-scale` differential all the same, against the SciGraphs arm;
  `scale.adaptive` names no record, because `adaptive.py`'s cut has nothing on this side to
  be compared with.
- It emits no snapshot and no column in the contract. Hints and journals are values a
  caller asks for; wiring them into the snapshot, the wasm ABI and the JS SDK is the merge
  step's work, and `docs/reports/phase-09-progress.md` lists it.

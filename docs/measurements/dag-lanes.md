# `layout.dag.lanes` — gates and measurement

The generic motor layout: one row per vertex in a topological order of the directed edges,
one lane per line of descent with lanes reused once free, and every edge a polyline of at
most two interior points. `O((n + m) log n)`, no dummy vertices, no budget to run out of.

Design: `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md`. Plan:
`docs/superpowers/plans/2026-10-05-dag-lanes-layout.md`. Verdict and its nine conditions:
`docs/decisions/dag-lanes.md`.

The motor knows no data source. Nothing under `crates/` names one, and the gate row
`no-source-names` keeps it that way on every run rather than once during implementation
(condition 8).

## Gates

Every row below was run on this branch. Real exit codes.

| gate | command | rc |
|---|---|---:|
| fmt | `scripts/orch/gr cargo fmt --all --check` | 0 |
| clippy | `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| tests | `scripts/orch/gr cargo test --workspace --no-fail-fast` | 0 |
| wasm32 | `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 |
| codegen | `scripts/orch/gr cargo run -q -p graph-cli -- codegen --check` | 0 |
| hashgate | `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 |
| roundtrip | `scripts/orch/gr cargo run -q -p graph-cli -- roundtrip --seeds 100` | 0 |
| capabilities | `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` | **1** |

`roundtrip` reports `layout.dag.lanes on its stated conventions on 100/100 seeds` over 5200
snapshots.

### Negative controls (a non-zero rc is the pass)

| control | rc | what it proved |
|---|---:|---|
| `GM_MUTATE_DAG_LANES_NODES=1 hashgate --seeds 8` | 1 | `DIVERGED layout.dag.lanes` and **nothing else** — the stage-scoped knob moves this stage only |
| `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` | 1 | the pre-existing shared-model control still goes red |
| broken `settle` (deliberate mutation, reverted) | 1 | all four `no_vertex_sits_on_an_edge` cases fail — condition 2's "a mutation, not a comment" |

### `capabilities --check` is red, and why

`--check` refuses a `gated` row whose hashgate or roundtrip record carries fewer than 1000
seeds (`capabilities/verdict.rs:13`, `MIN_SEEDS`). This worktree's gate runs are 8 seeds and
100 seeds — the sizes `scripts/orch/rows/dag-lanes.rows` uses — and running
`hashgate --seeds 1000` is forbidden to this job. Every `gated` row reports it, not this one
alone: `layout.dag.sugiyama`, `layout.packing.osage` and `transport.wasm.columnar` print the
same two lines. The record lives in `target/gates/`, untracked build output that this
worktree never had.

So the row is **UNKNOWN from this branch**, which the house rules score as a failure. It is
not a defect in the layout and it is not fixable here: it goes green the moment the
orchestrator's own 1000-seed gate run writes `target/gates/hashgate.json` and
`roundtrip.json`. Reported rather than papered over.

## Bench

`flock ~/goinfre/orch/bench.lock env GR_MEM=12g scripts/orch/gr cargo run -q --release -p
graph-cli -- bench --layout layout.dag.lanes --n 10000,100000,1000000 --repeat 3`

Three rounds, medians, `--release`, native 64-bit. Load before the run: **2.08**
(`/proc/loadavg` 1-minute). Preconditions checked first: no `develop-full.rows` process,
21 GB available, load under 14.

| n | edges | median | 1-minute load at start |
|---:|---:|---:|---:|
| 10 000 | 15 474 | 1.02 ms | 2.08 |
| 100 000 | 154 978 | 13.10 ms | 2.08 |
| 1 000 000 | 1 549 929 | 199.80 ms | 2.08 |

The `stress` column reads 0.55 / 0.54 / 0.53 across the three sizes — flat, so the cost is
not drifting superlinearly at the top of the range.

**`LANES_CEILING` stays at 1 000 000.** The plan lowers it only if 1M exceeds 1 s; it came in
at 199.80 ms, a factor of five inside the target. `registry/lanes.rs` labels the figure a
projection and says so in the `Ponytail (scale_ceiling)` clause (condition 5), and
`docs/measurements/phase09-ceilings.md` carries the row.

> **Caveat: the host is shared, so medians under load are upper bounds.**

### The `f32` half-row ceiling is a separate, harder bound

`row + 0.5` is exact in `f32` only while `row < 2^23 = 8 388 608`. Above that a polyline's
bend lands **exactly on an adjacent vertex row**, and the drawing is quietly wrong. Measured:
1M rows is comfortably inside it (7 388 608 rows of headroom below the declared ceiling), and
the guard is in code — `layout::lanes::run` refuses `node_count() >= MAX_ROWS` with
`StageError::Param` rather than drawing it wrong (condition 3, refusal form (a)). The boundary
is pinned by `a_graph_at_the_half_row_limit_is_refused_and_one_row_below_it_is_not`. That test
exercises the bound through `rows_fit` rather than by building two 8-million-vertex
topologies: `index_model` on that many string-keyed records does not fit the debug test
budget, and the guard is a single comparison whose arithmetic the predicate test states
exactly.

## What the gate model does not reach (condition 7)

The seeded gate model holds every `version` at 0.0 and points its arcs from a higher index to
a lower one, which is already a topological order. Over all 600 gate seeds that means the
drawing carries **zero** note-5 edges and never breaks a cycle: `hashgate` runs the
tie-break-by-index path and nothing else. The devil measured this before the build; it is
recorded in `registry/lanes.rs`'s `degradation` field rather than left for a reader to
rediscover.

The cycle-breaking and `version`-ordering paths are covered by unit tests instead, named in
the capabilities row's `oracle` string:

- `a_directed_cycle_is_broken_at_the_lowest_index_and_noted`
- `distinct_versions_break_a_cycle_and_the_heap_orders_by_version` — a seeded 5-vertex graph
  with distinct `version`s **and** a real cycle, added because the plan's version was one
  hand-built graph where version and index happened to agree. Here they disagree, so the
  test fails if the heap falls back to index order.
- `equal_versions_fall_back_to_index_order`

## Deliberate divergence: note 5 is carried by directed edges only

`layout.dag.sugiyama`'s own invariant checker (`acyclic.rs`) notes **every** non-loop edge
drawn head to tail, directed or not. `lanes` notes only the directed ones, because an
undirected edge has no head to tail to violate. On the plan's own
`parallel_and_undirected_edges_are_routed_without_notes` case an undirected edge *is* drawn
head to tail and carries no note. The drawing is still correct — `geometry.rs` reverses the
point order — and no consumer breaks: note 5's only readers are the renderer's name table and
`snapshot_cmd::dag.rs`, which is Sugiyama's own checker and never reads a lanes snapshot.

This is a divergence on a public note code, so it is stated in `registry/lanes.rs`'s
`degradation` field and not left implicit (condition 6).

## Oracle scope (condition 4)

`snapshot_cmd::hand_oracles::lanes` compares **all four columns** — node `x`, node `y`,
polyline `offsets` and every `pts` pair, and the notes as a whole `(code, index)` list — on
`to_bits()`, at the gate's own node counts (2 through 601 over the 100-seed sweep). Its
`oracle` metadata string says exactly that, rather than the plan's vaguer "compared bit for
bit per seed".

One control per column, each naming its column in the failure so a control that fired for the
wrong reason cannot pass for the right one:
`every_compared_column_has_a_control_that_catches_its_perturbation` perturbs `x`, `y`, one
interior point, one edge's whole span, and the note list.

## Real-history inputs are measured elsewhere

Commit-DAG lane widths against the reference tool's own column count belong to the
version-control plugin job, not here: the motor is not allowed to know what its inputs are.
`docs/decisions/dag-lanes.md` records that as its own scope boundary.

## What it does not do

Copied from the spec:

- No crossing reduction and no compaction across lanes. The width is greedy, not minimal.
- No time-proportional y. Rows are topological ranks, as in the reference tool's own log
  graph. A date axis is a separate question.
- No layered drawing of non-history DAGs better than Sugiyama's. It accepts any graph, but it
  is built for histories.
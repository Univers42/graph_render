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

`gate.sh target/rows-dag-lanes scripts/orch/rows/dag-lanes.rows` wrote
`target/rows-dag-lanes/summary.txt`: **16 of 17 rows PASS**, the one FAIL being this row.

`--check` refuses a `gated` row whose hashgate or roundtrip record carries fewer than 1000
seeds (`capabilities/verdict.rs:13`, `MIN_SEEDS`). This worktree's gate runs are 8 seeds and
100 seeds — the sizes `scripts/orch/rows/dag-lanes.rows` uses — and running
`hashgate --seeds 1000` is forbidden to this job. Every `gated` row reports it, not this one
alone: `layout.dag.sugiyama`, `layout.packing.osage` and `transport.wasm.columnar` print the
same lines. The record lives in `target/gates/`, untracked build output that this worktree
never had.

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
at 199.80 ms, a factor of five inside the target. `registry/lanes.rs`'s `Ponytail (scale_ceiling)`
clause names this measurement as its source (condition 5), and
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

## Commit DAGs

The motor knows no data source, so every input below is built in the version-control plugin at
`examples/plugins/git/`, on the host's `git`, and handed to the wasm motor as an ingest contract.
This replaces the placeholder that pointed here for work done elsewhere.

### Commands

```sh
# inputs (children-first logs in the plugin's FORMAT)
examples/plugins/git/git-log.sh /tmp/gitviz/contributor-stats.git > target/git-lanes/contributor-stats.log
examples/plugins/git/git-log.sh /tmp/gitviz/activitywatch.git    > target/git-lanes/activitywatch.log
examples/plugins/git/git-log.sh /tmp/gitviz/aw-server-rust.git   > target/git-lanes/aw-server-rust.log
examples/plugins/git/git-log.sh .                                > target/git-lanes/graph_render.log
examples/plugins/git/git-log.sh /tmp/gitviz/git.git              > target/git-lanes/git.git.log
scripts/orch/node-slim.sh node --experimental-strip-types examples/plugins/git/synthetic.mjs \
  1000000 1 target/git-lanes/synthetic-1000000.log

# the motor
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown

# times, 3 rounds, inputs alternating round by round, medians. The synthetic runs at DRUN_MEM=12g.
flock ~/goinfre/orch/bench.lock env DRUN_MEM=12g \
  scripts/orch/node-slim.sh node --experimental-strip-types examples/plugins/git/bench.mjs \
  target/wasm32-unknown-unknown/release/graph_wasm.wasm <logs...>

# the width the reference drawing uses
git -C <repo> log --all --topo-order --graph --format=%H > target/git-lanes/<name>.graph
scripts/orch/node-slim.sh node --experimental-strip-types examples/plugins/git/graph-width.mjs \
  target/git-lanes/<name>.graph

# the width with the plugin on committer time, and git's own date order. Same motor, same tree;
# only the plugin's format string differs. Times are unchanged, so this is one round, widths only.
flock ~/goinfre/orch/bench.lock scripts/orch/node-slim.sh node --experimental-strip-types \
  examples/plugins/git/bench.mjs \
  target/wasm32-unknown-unknown/release/graph_wasm.wasm target/git-lanes/<name>.log   # -> layout.dag.lanes.width=<k>
git -C <repo> log --all --date-order --graph --format=%H > target/git-lanes/<name>.date.graph
scripts/orch/node-slim.sh node --experimental-strip-types examples/plugins/git/graph-width.mjs \
  target/git-lanes/<name>.date.graph
```

`bench.mjs` times `parseLog`, `toRows` + `rowsToIngest`, `JSON.stringify`, `motor.buildContract`,
then each registered layout, over 3 internal rounds, and reports medians. For `layout.dag.lanes`
it also prints `layout.dag.lanes.width=<k>`, the number of distinct node `x` values, read off the
JSON face's `geometry.nodes.x` — the flat `f32` column of a `Point` node geometry
(`geometry.nodes.kind` is `"Point"`, `nodes.id` is the parallel id column). `graph-width.mjs` gives
the reference's own width: the maximum over lines of `(index of "*") / 2 + 1`, since
`git log --graph` draws one two-character column per commit lane.

### Results

Load before each run was printed by `bench.mjs`: **7.55**, **7.15**, **6.82** for the three rounds
of the real histories and **6.12**, **5.95** for the synthetic's rounds. Times are wasm32 only.

| input | n | m | build ms | lanes ms (wasm32) | sugiyama ms (wasm32) | lanes width (author time) | lanes width (committer time) | `git log --graph` width | `git log --graph --date-order width` | note 5 | note 4 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| contributor-stats | 488 | 490 | 2.2 | 0.1 | 0.8 | 2 | 2 | 2 | 2 | 0 | 0 |
| activitywatch | 1 271 | 1 356 | 5.7 | 0.2 | 28.1 | 6 | 6 | 4 | 4 | 0 | 0 |
| aw-server-rust | 989 | 1 014 | 4.7 | 0.1 | 3.5 | 14 | 8 | 3 | 4 | 0 | 0 |
| graph_render | 2 175 | 2 860 | 9.3 | 0.4 | 224.9 | 40 | 40 | 26 | 36 | 0 | 0 |
| git/git | 85 928 | 107 694 | 570.1 | 14.7 | 111.3 | 364 | 281 | 106 | 181 | 0 | 0 |
| synthetic (n = 10⁶, seed 1) | 1 000 000 | 1 049 603 | 5 917.6 | 186.7 | 763.8 | 75 | n/a | n/a | n/a | 0 | 0 |

The synthetic has no `git log --graph` width in any column: it is not from a repository.
The `lanes width` columns are the lane count it drew; the two `git log --graph` width columns are
the reference's own, on the real histories, once in git's topo order and once in git's
`--date-order`. Times do not depend on the plugin's format, so the time columns and the topo-order
`git log --graph` width are from the first run and were not re-taken; only the two new columns were
re-measured.

### Why the two `lanes width` columns differ

`lanes width (author time)` and `lanes width (committer time)` are the **same motor, the same tree,
the same log, one round each** — they differ only in the plugin's format string, `%at` against
`%ct`. The plugin feeds that field to the records as `updatedAt`, which the SDK's rows adapter maps
onto `version`, and `layout.dag.lanes` orders rows by version. A rebased or cherry-picked commit
keeps its old author time, so ordering by author time interleaves branches and spends more lanes
than the history needs. Committer time is the time the commit was written, and is what git's own
`--date-order` sorts on. On aw-server-rust the gap is 14 lanes to 8, on git/git 364 to 281.

The `git log --graph --date-order width` column is the floor to read those against: git's own
drawing in git's own date order. The committer-time column still sits above it (40 against 36 on
graph_render, 281 against 181 on git/git), so switching the format recovers a large part of the
overdraw but does not close it — the residual is the motor's greedy lane assignment, cause 1 of
`docs/decisions/dag-lanes-merge.md`, not the plugin's time field.

### Targets

| target | measured | verdict |
|---|---:|---|
| git/git ≤ 50 ms in wasm32 | 14.7 ms | **PASS**, a factor of 3.4 inside |
| 1M synthetic ≤ 1 s in wasm32 | 186.7 ms | **PASS**, a factor of 5.4 inside |
| every edge routed (zero note 4) | 0 on all six inputs | **PASS** |

The largest `n` that completed is **1 000 000**, at 186.7 ms median over 3 rounds — no larger `n`
was tried and none was needed, so nothing stopped a bigger run.

`layout.dag.sugiyama` carries note 4 on two inputs (11 310 on git/git, 28 821 on the synthetic):
its dummy-vertex budget ran out, which is exactly the failure `layout.dag.lanes` removes. On
git/git it is also the slower of the pair — 111.3 ms against 14.7 ms, so `lanes` is 7.6x faster
there — and on graph_render the gap is 224.9 ms against 0.4 ms.

> **Caveat: the host is shared (1-minute load 4.9 to 7.6 throughout), so every median here is an
> upper bound.** Container start-up (`node-slim`, `gr`) is outside all of them. Only the layout
> time is a motor number; `build ms` is dominated by `JSON.stringify`/`buildContract` over a
> 224 MB contract at 1M and is not part of either target. The two `lanes width` columns differ
> **only in the plugin's format**: the same motor, the same tree, the same log, one round each, and
> nothing under `crates/` differs between them — so the only variable is whether the plugin feeds
> author time or **committer time**. The synthetic's shape is the plugin's own model
> (`synthetic.mjs`): no octopus merges, no cherry-picks, no clock skew, and its committer time
> (`%ct`) is strictly increasing where a real history's is not. The `graph_render` rows are the
> one input that did not reproduce exactly: the author-time run read this tree at 2 175 commits,
> the committer-time run at 2 212, so its two widths come from slightly different inputs. The
> committer-time and date-order widths are single rounds, not medians of three, because times were
> not re-taken; treat them as one sample, not a distribution.

## What it does not do

Copied from the spec:

- No crossing reduction and no compaction across lanes. The width is greedy, not minimal.
- No time-proportional y. Rows are topological ranks, as in the reference tool's own log
  graph. A date axis is a separate question.
- No layered drawing of non-history DAGs better than Sugiyama's. It accepts any graph, but it
  is built for histories.
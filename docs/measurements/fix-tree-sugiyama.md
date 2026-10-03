# fix-tree-sugiyama — review-layout-tree L-02, L-03, L-12, L-13, L-14 + the sg-sugiyama row

Source: `docs/reviews/review-layout-tree.md`. Brief: `prompts/jobs/fix-tree-sugiyama.md` over
`prompts/jobs/fix-common.md`. Paths: `crates/graph-core/src/layout/sugiyama/**`, the SUGIYAMA
row's prose in `crates/graph-core/src/registry/grid.rs`. Reference: `SciGraphs/core/
scigraphs_core/mesh/layouts/hierarchical.py`.

One structural move outside the findings: `mod.rs`'s `#[cfg(test)] mod measurement` (the
crossing-count dump, 149 lines) became `sugiyama/measurement.rs`, because `mod.rs` crossed
the 300-line house limit once `layered` grew its `Result`. Nothing in it changed but its
indentation; the dump it writes is byte-identical.

Settled and not reopened: the review's "no Brandes-Köpf" item. The reference's X phase is the
Sugiyama-Tagawa-Toda priority method with the same `_PRIORITY_NODE_BUDGET = 200000`
(`hierarchical.py:8-10,565-580`); `coords.rs` ports it.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| L-02 | MAJOR | fixed | `layout::sugiyama::ordering::tests::a_wide_layer_computes_each_median_once_and_not_once_per_comparison` | test `ordering/tests.rs:85`; fix `ordering.rs:144` (`sort_by_median`), keys precomputed into `Vec<(f64, slot, v)>` |
| L-03 | MAJOR | fixed | `layout::sugiyama::ordering::tests::transpose_fills_a_neighbour_buffer_once_per_pair_and_not_ten_times` | test `ordering/tests.rs:104`; fix `ordering/transpose.rs:38` (`Sorted`), `:104` (`transpose`), `:123` (`transpose_row`) |
| L-12 | MINOR | doc-only (the note half is **deferred**, see below) | `layout::sugiyama::coords::tests::above_the_priority_budget_leaves_x_at_the_raw_slot_index_and_the_snapshot_does_not_say_so` | test `coords.rs:258`; seam `coords.rs:43` (`build_within`); text `registry/grid.rs:107` (`degradation`), `coords.rs:15` |
| L-13 | MINOR | fixed | `layout::sugiyama::ordering::tests::a_layer_count_below_a_vertices_own_layer_is_refused_rather_than_dropping_it` | test `ordering/tests.rs:124`; fix `ordering.rs:70` (`build`), `:91` (`layer_covered`), callers `mod.rs:42`, `scaled.rs:87` |
| L-14 | MINOR | doc-only | `layout::sugiyama::routing::tests::a_self_loop_carries_no_interior_points_and_is_left_to_the_style_post` | test `routing/tests.rs:33`; doc `routing.rs:10`, `registry/grid.rs:95` (`oracle`) |
| sg-sugiyama row | — | doc-only | `registry::tests::sugiyama_declares_polyline_edges_and_the_reference_dummy_budget` (`registry/tests.rs:44`, asserts `complexity.contains("heuristic")` at `:49`) | `registry/grid.rs:95` (`oracle`, `complexity`, `degradation`) |

## L-02 — one median per movable vertex, not one per comparison

The comparator called `median_position` twice per comparison, and each call collects a `Vec`
and sorts it. The fix builds the key once per movable vertex: `(median, slot, v)`, sorted by
`(median, slot)` — the same tie rule the comparator had (slot ascending), so the order is
unchanged, and the ranked vertices still land on the same ascending set of movable slots, so
a neighbourless vertex stays put.

RED (old comparator, test-only counter around `median_position`):

```
layout::sugiyama::ordering::tests::a_wide_layer_computes_each_median_once_and_not_once_per_comparison ... FAILED
  153370 medians for 2000 vertices over eight sweeps
```

GREEN:

```
running 5 tests
.....
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 1242 filtered out; finished in 0.05s
```

## L-03 — one neighbour buffer per vertex, reused for the pass

`pair_crossings` collected and sorted two `Vec`s per call and was called four times per
adjacent pair. `ordering/transpose.rs` now holds one sorted buffer per vertex per direction
(`Sorted`), refilled only after a swap has invalidated it; the `after` count of a pair reuses
what the `before` count filled, because nothing moved in between. Invalidation is per swap,
not per vertex — one swap drops every buffer, which the module's `Ponytail:` line names as
the case where this is no cheaper than before (time only, never a crossing count).

RED:

```
layout::sugiyama::ordering::tests::transpose_fills_a_neighbour_buffer_once_per_pair_and_not_ten_times ... FAILED
  500256 fills over 63936 examined pairs (7.82 per pair)
```

GREEN: the same 5 tests pass, the bound being 3 fills per examined pair.

## Output unchanged

The 7 fixtures plus the 230 synthetic DAGs of the crossing dump are byte-identical, and
`hashgate --seeds 8` is 4-way equal on 8/8 seeds, so every registered output is bit-identical
native vs wasm32.

```
scripts/orch/gr cargo test -q -p graph-core --lib dump_crossing_measurements -- --ignored
cmp /tmp/opencode/fx/dag-crossings-before.json target/dag-crossings.json   # no output: identical
```

## Bench, the largest-layer case

`graph-cli bench --layout layout.dag.sugiyama --n 10000,100000 --repeat 3`, native
`--release`, one host. **This host's wall clock is noisy** — the same binary varied 70-117 ms
at 10k and 199-359 ms at 100k across four runs — so the timings below are indicative and the
load-bearing numbers are the two work counts, which are exact and reproducible:

| measure | before | after |
|---|---|---|
| medians over a 2000-vertex two-layer sweep | 153 370 | <= 32 000 (`16n`) |
| transpose buffer fills per examined pair | 7.82 | <= 3 |
| bench, n = 10 000 | 99.41 ms | 70.3 / 117.3 / 107.3 / 112.6 / 74.6 ms |
| bench, n = 100 000 | 495.01 ms | 220.2 / 358.6 / 201.4 / 198.6 / 213.9 ms |
| bench `stress-1`, n = 10 000 / 100 000 | 0.5977 / 0.6117 | 0.5977 / 0.6117 (every run) |

`stress-1` is unchanged at both sizes in every run, which is the independent check that the
drawings are the same and only faster.

```
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.dag.sugiyama --n 10000,100000 --repeat 3
```

## L-12 — the X phase above the budget is still silent; the note half is deferred

The finding is half observation, half fix. The observation holds and is now pinned by a
test: above `PRIORITY_NODE_BUDGET` (`coords.rs:22`, the reference's own value) `passes` is 0,
x is exactly the raw ordering slot index, and nothing in the snapshot says so. `Coords` now
takes the ceiling as a parameter (`build_within`, the way `Layering::build` takes
`DUMMY_BUDGET`), so a test reaches the skip without a 200k-vertex fixture.

The note half is **deferred**: no existing code fits. The notes section is a closed set of
five codes (`graph-contract`'s `notes.rs:36-55`) and a skipped phase is snapshot-wide, which
only code 3 (`packing.approximate`) may be (`notes.rs:174-177`) — reusing it would lie, and
code 4 is edge-indexed. A new code is a contract minor bump whose closed-set lists live in
four files outside this job's paths (`notes.rs`, `canonical_json/schema.rs:176-183`,
`notes/tests.rs:54-79`, `docs/contract/binary-layout.md:129-136`) and would move the
`hashgate` exercise seeds (`graph-cli/src/snapshot_cmd/exercise.rs:149-160`). Recommendation
under "decisions needed" below. Until then the behaviour is documented where a consumer reads
it: the registry row's `degradation`.

## L-13 — a vertex out of range is refused, not dropped

`Ordering::build` returns `Result<Self, StageError>` and refuses a `num_layers` below any
vertex's own layer with `StageError::Param { name: "num_layers", .. }`. Every public input
gets its layer count from `max() + 1`, so no `Topology` can reach the refusal — the test is
the seam test `fix-common` asks for, and says so in its doc comment. `run` and `run_scaled`
propagate with `?`; nothing else can produce it.

RED: the old code panicked, which is the stronger form of the review's "silently dropped"
(it dropped only where the drop did not index past the end):

```
layout::sugiyama::ordering::tests::a_layer_count_below_a_vertices_own_layer_is_refused_rather_than_dropping_it ... FAILED
  panicked at crates/graph-core/src/layout/sugiyama/ordering.rs:109:26:
  index out of bounds: the len is 2 but the index is 2
```

## L-14 — the self-loop convention, documented rather than changed

The reference emits no edge geometry at this stage: `_sugiyama_layout` returns an `(N, 3)`
position matrix (`hierarchical.py:651-652`). The loop arc belongs to the router that runs
after the layout, and the reference emits one exactly when a polyline's two endpoints
coincide (`SciGraphs/core/scigraphs_core/mesh/edge_styles.py:458-459`), which is what an empty
`Polyline` row is on this side too — the contract draws an empty row straight, and the motor's
style post turns that coincident segment into the loop. So the degenerate polyline is the
intended convention, matching the reference; it is now documented at `routing.rs:10` and in
the registry row's `oracle`, and the test pins it. Inventing arc points at layout time would
spend a stage's budget on geometry no oracle has.

## The sg-sugiyama row

`oracle` and `ponytail` already said what the cycle step does now — orienting every non-loop
edge forward along the dense node order, not a greedy feedback-arc-set peel — because
`sg-sugiyama` reworded them on this branch. What was still wrong is `complexity`: "O(n+m) per
phase" is false for crossing reduction, which sorts every layer's median keys and the
transpose's neighbour positions. It now names the sweep's own cost and keeps the word
`heuristic` that `registry/tests.rs:49` asserts on. No `Metadata` value codegen or
`capabilities` reads moved (`tier`, `scale_ceiling`, the two geometry kinds), and the
`capabilities --check` output is byte-identical to the pre-change run.

## Commands

| command | exit | last lines |
|---|---|---|
| `scripts/orch/gr cargo fmt --all --check` | 0 | clean |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 | `Finished dev profile` |
| `scripts/orch/gr cargo test --workspace --no-fail-fast` | 101 | one pre-existing failure, see below; every other target ok |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 | — |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 | `layout.dag.sugiyama: 4-way equal on 8/8 seeds`, `PASS` |
| `scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8` | 1 | `FAIL: 8 of 8 seeds diverge` (the negative control) |
| `scripts/orch/gr cargo run -q -p graph-cli -- codegen --check` | 0 | `up to date docs/contract/snapshot-schema.json` |
| `scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check` | 1 | `73 rows, 36 problems` — identical output to before the change; the 36 are the pre-existing `hashgate --seeds 1000` / no-record problems |
| `scripts/scigraphs-conformance.sh` | 0 | `SUGIYAMA: ok — 597 f64, 1020 f32 of 1020 coordinates, median 1.259e-16 <= 1.000e-15`, `PASS` |
| `cargo test -p graph-core --lib dump_crossing_measurements -- --ignored` | 0 | dump byte-identical to the pre-change run |
| `cargo run --release -p graph-cli -- bench --layout layout.dag.sugiyama --n 10000,100000 --repeat 3` | 0 | 74.64 ms / 213.89 ms (best of five runs; the host is noisy) |

`cargo test --workspace --no-fail-fast` exits 101 on one test outside this job's paths and
unrelated to it: `layout::graphviz::dot::rank_tests::the_first_twenty_fixture_seeds_rank_as_
the_oracle_ranks_them` panics because `target/probe/rank1000.txt` is absent from this
checkout. Its own module doc says the probe is generated by
`python3 target/probe/rank_oracle.py --digest target/probe/rank1000.txt` inside
`ge-graphviz-oracle` and that a missing measurement file should skip rather than fail. It
fails identically before this job's changes; reported, not touched.

## Test-only counters

`ordering.rs` and `ordering/transpose.rs` carry two `#[cfg(test)]` thread-local counters (median
evaluations, transpose buffer fills) so the two cost findings have a RED that counts work
instead of timing it. They are not compiled outside `cfg(test)`, read nothing, write nothing,
and touch no output path.
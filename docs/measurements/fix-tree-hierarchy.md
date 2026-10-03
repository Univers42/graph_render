# fix-tree-hierarchy — L-09, L-10, L-11, L-15, L-16, L-17, L-18, L-19

Source: `docs/reviews/review-layout-tree.md`. Brief: `prompts/jobs/fix-tree-hierarchy.md` over
`prompts/jobs/fix-common.md`. Paths: `layout/hierarchy*`, `layout/tidy_tree*`, `layout/treemap*`,
`layout/hierarchical_3d*` under `crates/graph-core/src/`. Branch `fix-tree-hierarchy`,
worktree `$GM_SCRATCH/wt/fix-tree-hierarchy`, 2026-10-03.

No registered layout output moved: six of the eight ids are documentation, the two that touch
code are a test-only fixture boundary and a `debug_assert!`. `hashgate --seeds 8` is 4-way
equal on every row (below).

| id | severity | verdict | RED | GREEN | file:line |
|---|---|---|---|---|---|
| L-09 | MINOR | doc-only (rung 1) | none needed — a doc claim | module doc now states O(n + m log m) and names both comparison sorts | `layout/hierarchy.rs:24-29` |
| L-10 | MINOR | doc-only | none needed — no disagreement found | the one-deep offset documented at `depth()` with both consumers and the conformance row that clears them | `layout/hierarchy.rs:132-158` |
| L-11 | MINOR | fixed | `hierarchy::fixture::tests::an_overflowing_exponent_is_refused_and_names_the_node` (and its NaN / -inf siblings) — 3 FAILED, "did not panic as expected" | `weight()` refuses a non-finite parse and names the node | `layout/hierarchy/fixture.rs:47-69`, `fixture/tests.rs:21-41` |
| L-15 | MINOR | **false** | the test passes on the unfixed code: `total_cmp` reports two `+inf` equal, which is d3's outcome | test kept as the evidence; `sorted_children`' doc states why the two comparators cannot differ here | `layout/treemap/rows.rs:36-51`, `treemap/tests/mod.rs:82-136` |
| L-16 | MINOR | fixed | the invariant pin fails when inverted (both halves, two controls) — the code is correct, the *assertion* was missing | test + `debug_assert_eq!` on `order()[0]` + a doc naming the invariant | `layout/tidy_tree/walk.rs:180-219`, `tidy_tree/tests/mod.rs:181-217` |
| L-17 | MINOR | doc-only | none known — and no seam to build one (see below) | `step_contour` documents that the two `expect`s mirror `tree.js`'s own unguarded reads | `layout/tidy_tree/walk/contour.rs:45-74` |
| L-18 | MINOR | doc-only | none — the reference does the same | `disk`'s module doc states the singleton-at-origin convention and why `radius` is discarded | `layout/hierarchical_3d/disk.rs:1-15` |
| L-19 | MINOR | doc-only | none needed — the framing was already gone from this tree | module doc names BFS depth vs Sugiyama longest-path layering, with the `1 -> 0 -> 2` case they disagree on | `layout/hierarchical_3d.rs:24-37` |

## Notes per row

- **L-09.** The doc promised a counting sort; the code sorts notes once (`Hierarchy::of`, at
  most `2m` entries) and cycle cuts once (`break_cycles`, at most one per disjoint cycle, so
  at most `n`), both with `sort()`. Rung 1 taken: the doc now says O(n + m log m) and why a
  counting sort would not buy it back — the note sort's key is `(code, index)` and `index` is
  a dense id, so a second counting key would be needed anyway. Rung 2 was unavailable: there
  is no `memory.rs` row about sort cost (`crates/graph-core/tests/memory.rs:126`
  `hierarchy_layout_pipeline_memory_per_node` measures memory per node, not sort work), so no
  measurement says a counting sort is cheaper.

- **L-10.** Every `depth()` caller, and what it does with the value:
  `layout/circular.rs:72` (`rings[v] = hierarchy.depth(v)`, a **0-based ring index**),
  `crates/graph-cli/src/snapshot_cmd/hand_oracles.rs:55` (same, its own `rings` column),
  `layout/tidy_tree/walk.rs:231,242,245` (`normalize`: an argmax and `y = depth * ky`),
  `layout/hierarchy.rs:164` (`max_depth`), `crates/graph-core/src/analysis/depth/hierarchy.rs:45,107`
  (asserts the analysis agrees), plus tests. The two ring-index consumers are the only place
  the offset could bite, and **neither disagrees with its reference**: SciGraphs conformance
  row 32 `CIRCULAR_HIERARCHY` is green and "`f32`-identical on all 1020 coordinates"
  (`docs/measurements/scigraphs-conformance.md:157`, re-measured in this job's run: `509 f64,
  1020 f32, median 4.320e-16 <= 1.000e-15`), so the one-deep forest shift is the oracle's own
  convention. Verdict `doc-only`: the offset is deliberate (module doc line 21-22) and is now
  documented where the value is read.

- **L-11.** The reachable non-finite weight is an **overflowing exponent**, not a `NaN`
  literal: the canonical JSON number grammar admits no `NaN`/`Infinity` token
  (`graph_contract::canonical_json::parse:32`), so `"weight": "NaN"` is a *string* (caught by
  the wrong-arm panic) and bare `NaN` does not parse at all. `"weight": 1e999` does parse, to
  `Number("1e999")`, and `f64::from_str` makes that `+inf` — which reaches
  `treemap::rows::node_values`' sum and every ratio below it. `weight()` now refuses it and
  names the node; the test module drives the private `node()` because `load()` only reaches the
  four checked-in files, and `every_checked_in_fixture_still_loads` keeps the other arm honest.
  `load`'s signature is unchanged (`pub(crate)`, test-only, seven callers outside this job's
  paths): a fallible fixture loader is a separate decision, not this finding.

  RED (finiteness check removed, everything else in place):

  ```
  test layout::hierarchy::fixture::tests::a_negative_infinity_weight_is_refused_and_names_the_node - should panic ... FAILED
  test layout::hierarchy::fixture::tests::a_nan_weight_is_refused_and_names_the_node - should panic ... FAILED
  test layout::hierarchy::fixture::tests::an_overflowing_exponent_is_refused_and_names_the_node - should panic ... FAILED
  note: test did not panic as expected at crates/graph-core/src/layout/hierarchy/fixture/tests.rs:37:4
  test result: FAILED. 2 passed; 3 failed; 0 ignored; 0 measured; 1244 filtered out; finished in 0.00s
  ```

  GREEN: `test result: ok. 5 passed; 0 failed; ... 1244 filtered out`.

- **L-15.** The finding is **refuted**. `f64::total_cmp` reports two `+inf` as `Equal`
  (IEEE 754 total order), and `sort_by` is stable, so the order is kept; d3's
  `b.value - a.value` is `inf - inf == NaN`, which `Array.prototype.sort` coerces to `+0`
  (ECMA-262 `SortCompare`: "If v is NaN, return +0") — keep order as well. They agree. The
  one input on which they would differ is a `NaN` aggregate, and the aggregate is a sum of
  `clamp_weight` outputs (`treemap.rs:52`), so it is never `NaN` and never `-0.0`; `+inf` is
  reachable, and the test reaches it for real: `a` and `b` each carry `1e308` own weight *and*
  a `1e308` child, so `node_values` computes `inf` exactly as a real aggregate would. The test
  also re-sorts the same slice through an emulated d3 comparator with the `NaN -> +0`
  coercion (`js_order`) and requires both orders to be `[1, 3, 5]`. The test passed on the
  unfixed code, so it is the evidence and stays. Separately: the doc cited
  `treemap/squarify.js` for the comparator, and that file contains **no sort** — the primitive
  is `hierarchy/sort.js:1-7`, which takes the comparator as an argument (the treemap consumes
  `parent.children` as given, `squarify.js:8`). The doc now says so.

- **L-16.** `second_walk` iterates `Hierarchy::order()` (breadth-first, `hierarchy.rs:259-275`)
  where `tree.js` runs `t.eachBefore(secondWalk)` (preorder). The output is identical only
  because the loop reads **nothing but `m[parent]`** — no sibling, no cousin, no descendant —
  so it needs each parent *written* before its child is *read*. Two facts deliver that and
  both are now pinned: `order()[0] == root` (what `skip(1)` drops) and every other node's
  **layout** parent precedes it in `order()`. The test recovers the layout parent from
  `children`, not `Hierarchy::parent` — the latter is `None` for a root hung off the virtual
  root, which is exactly the forest case. `debug_assert_eq!` checks the first fact on every
  run. Negative controls, both inverted and both observed failing:

  ```
  # order()[0] vs order[1]:
  assertion `left == right` failed: CONTROL: not the root   left: 5   right: 0
  # at(parent) < at(v) inverted:
  layout parent 5 of 0 is visited after it, so m[parent] is not final
  ```

  Restored: `test layout::tidy_tree::tests::a_two_root_forest_orders_the_root_first_and_every_layout_parent_before_its_child ... ok`.
  `preorder(self.h, root)` would be arithmetically equivalent and would allocate per run, so
  the order stays and the invariant is what is asserted; the doc says so.

- **L-17.** **No RED input is known and no unit seam exists** for one: `apportion`,
  `step_contour` and `finish_contour` are private to the `impl<'h> Walk<'h>` in this module
  tree and each needs a whole `Walk` mid-contour, so there is no seam to drive the failure
  path from a test. Two reasons the GREEN as written is not the right change:
  1. **the reference has the same failure mode.** `tree.js`'s loop condition advances
     `vim`/`vip`, then steps the *outside* cursors unguarded (`vom = nextLeft(vom); vop =
     nextRight(vop);`) and dereferences them on the next line (`som += vom.m`). An exhausted
     cursor is `undefined` there — a `TypeError`; it is a panic here, on the same line. A
     `StageError` would *depart* from the oracle, which `docs/contract/` and the oracle win
     over the reviewer's guess.
  2. **the error type cannot name it.** `StageError` is `Copy` over four fixed variants
     (`crates/graph-core/src/stage.rs:35`: `Capacity`, `NonFinite`, `Param`, `Snapshot`), and
     none fits a broken hierarchy invariant — `Param` would report a proof failure as a bad
     parameter. A new variant lives in `stage.rs`, outside this job's paths.

  Verdict `doc-only`: the change is the `step_contour` doc, which now states the obligation,
  the reference's identical exposure, and why the error variant is a separate decision.

- **L-18.** **The reference places a singleton level at the origin.**
  `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:95-96` returns `[(0.0, 0.0)]` at
  `count == 1`, before `rings`, `radius` or any ring point is computed; its caller's
  `radius = scale * 0.5 * np.sqrt(count / widest)` (`:142`) is therefore computed and discarded
  for a one-node level, and `disk(count, radius)` here discards it the same way. The general
  path would give `ring_points(0, 1, r, 1) == (0.5 * radius, 0)` (`disk.rs:113-122`), so the two
  branches disagree — the reference disagreeing with itself, which is what a port copies.
  Conformance row 17 `HIERARCHICAL_3D` is green node for node (`374 f64, 1020 f32, median
  9.029e-17 <= 1.000e-16` in this run) and
  `tests::a_single_node_sits_at_the_centre_of_the_negative_end` (`tests.rs:90`) already pins
  the placed output. Branch kept, convention documented in `disk`'s module doc.

- **L-19.** The premise does not hold on this tree: `layout/hierarchical_3d.rs` carried no
  Sugiyama framing to fix — line 1-2 already reads "SciGraphs' own `_hierarchical_layout_3d`
  ... ported whole", and `git grep -n "Sugiyama|sugiyama|lift|2D"` over the module and its
  children matches only one *test* doc (`tests.rs:22`, "a 2D snapshot would answer all of
  them"). What the finding actually asks for — that a reader know which depth a level is — is
  added: the doc now states that nothing here reads `layout::sugiyama`, that the levels are
  this module's undirected BFS depth, and the case where the two definitions disagree
  (`1 -> 0 -> 2`: Sugiyama layers `1: 0, 0: 1, 2: 2`, BFS depths `0: 0, 1: 1, 2: 1`, because
  the root is the component's diameter midpoint, not an in-degree-0 source).

## Commands

```
scripts/orch/gr cargo test -p graph-core --lib hierarchy::fixture            -> 1 (RED, 3 failed)
scripts/orch/gr cargo test -p graph-core --lib hierarchy::fixture            -> 0 (GREEN, 5 passed)
scripts/orch/gr cargo test -p graph-core --lib a_two_root_forest             -> 1 (control: order()[0])
scripts/orch/gr cargo test -p graph-core --lib a_two_root_forest             -> 1 (control: inverted parent order)
scripts/orch/gr cargo test -p graph-core --lib a_two_root_forest             -> 0 (restored)
scripts/orch/gr cargo test -p graph-core --lib treemap                       -> 0 (20 passed)
scripts/orch/gr cargo test -p graph-core --lib tidy_tree                     -> 0 (24 passed)
scripts/orch/gr cargo fmt --all --check                                      -> 1 (3 hunks) then 0 after `cargo fmt --all`
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings       -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                        -> 101 (one pre-existing failure, below)
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown    -> 0
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8              -> 0 (PASS, 4-way equal on 8/8)
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8 -> 1 (FAIL: 8 of 8 seeds diverge)
scripts/scigraphs-conformance.sh                                             -> 1 (two reference-byte rows, below)
```

Last lines:

```
hashgate --seeds 8:            4-way equal on 8/8 seeds
PASS
hashgate --seeds 8 (control):  4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge
scigraphs-conformance.sh:      HIERARCHICAL_3D: ok — 374 f64, 1020 f32 of 1020 coordinates, median 9.029e-17 <= 1.000e-16
                               SUGIYAMA: ok — 597 f64, 1020 f32 of 1020 coordinates, median 1.259e-16 <= 1.000e-15
                               CIRCULAR_HIERARCHY: ok — 509 f64, 1020 f32 of 1020 coordinates, median 4.320e-16 <= 1.000e-15
                               YIFAN_HU: FAIL — reference bytes are not the pinned ones (sha 78ccfd5e...)
                               GRAPHVIZ_SFDP: FAIL — reference bytes are not the pinned ones (sha 78ccfd5e...)
                               FAIL
```

## Red rows this job did not cause, and did not touch

Both are outside the paths this job may edit, and neither is a motor-arm failure.

1. **`scripts/scigraphs-conformance.sh` exits 1 on two reference-byte pins.**
   `YIFAN_HU` and `GRAPHVIZ_SFDP` share one Graphviz `sfdp -Tplain` reference (documented at
   `docs/measurements/scigraphs-conformance.md:203`) and this host produces
   `sha256(ref/*.f64) = 78ccfd5e6306be74450d906fbf60ba47a0d1df124d4baa2a4488b39cb3229a72`,
   while the pin is `4c90e6c3493b207255f3be114f517f41ceb234b7ff75e241a85991b400b9144f`
   (`crates/graph-cli/src/oracle_python/conformance/baseline/table/graphviz.rs:15`). That file
   is written by `harness/oracle-graphviz.py` from the pinned Graphviz 16.1.0
   (`(20260904.0139)`, as `ref/GRAPHVIZ_SFDP.json` records) and no Rust code in this motor
   touches it; the judge fails it on check 2, before any motor comparison. Both rows are
   `layout.force.yifan_hu` / `layout.force.sfdp`, which this job does not edit. **Recommendation:
   re-pin the two rows from a run on a host whose Graphviz matches the pin, in the job that
   owns `crates/graph-cli/src/oracle_python/conformance/baseline/**` — not here.**

2. **`layout::graphviz::dot::rank_tests::the_first_twenty_fixture_seeds_rank_as_the_oracle_ranks_them`
   fails**: `target/probe/rank1000.txt` does not exist in this worktree, and
   `graphviz/dot/oracle_probe.rs:57-62` panics on the missing probe file. Its own module doc
   says a missing measurement file should *skip* rather than fail, so the panic is a second
   inconsistency in the same file — both are outside this job's paths.
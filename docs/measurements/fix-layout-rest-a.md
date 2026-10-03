# fix-layout-rest-a — spiral, circular, coords, planarity and the layout root

Source: `docs/reviews/review-layout-rest.md`, ids `LR-01`, `LR-12` (the `layout/circular.rs:3`
half only), `LR-13` … `LR-19`, `LR-24` … `LR-30`, `LR-34` … `LR-40`. Brief:
`prompts/jobs/fix-layout-rest-a.md` over `prompts/jobs/fix-common.md`. Paths under
`crates/graph-core/src/`. Worktree `fix-layout-rest-a`, 2026-10-03. The 3-D, grid and random
ids are job `fix-layout-rest-b` and were not read.

| id | severity | verdict | test name | file:line |
|---|---|---|---|---|
| LR-01 | MAJOR | fixed | `layout::spiral::tests::a_resolution_that_cannot_produce_finite_geometry_is_refused` | `layout/spiral.rs:121-134`, `layout/spiral/tests.rs:148-179` |
| LR-12 | MINOR | doc-only | — | `layout/circular.rs:3` |
| LR-13 | MINOR | doc-only | — | `layout/circular/ring.rs:3-5` |
| LR-14 | MINOR | fixed | `layout::coords::tests::a_non_finite_coordinate_poisons_the_limit_rather_than_dividing_the_cloud`, `a_ragged_pair_of_columns_is_refused_rather_than_half_transformed` | `layout/coords.rs:37-52`, `:98-115`, `layout/coords/tests.rs:108-137` |
| LR-15 | MINOR | doc-only | — | `layout/coords.rs:80-86` |
| LR-16 | MINOR | fixed-by LR-01 | (LR-01's test, `n` 0 and 1 included) | `layout/spiral.rs:102` |
| LR-17 | MINOR | doc-only | — | `layout/spiral.rs:84-94` |
| LR-18 | MINOR | fixed | `layout::spiral::tests::a_runner_that_hands_a_short_out_is_refused_rather_than_truncated` | `layout/spiral.rs:181-190`, `layout/spiral/tests.rs:186-198` |
| LR-19 | MINOR | doc-only | — | `layout/spiral.rs:7-10` |
| LR-24 | MINOR | fixed | `layout::planarity::tests::properties::no_small_graph_reaches_an_expect_on_the_public_planarity_path` | `layout/planarity.rs:159-183`, `layout/planarity/tests/properties.rs:7-57` |
| LR-25 | MINOR | fixed | same property sweep (the new `debug_assert!` is what it holds) | `layout/planarity/embed.rs:16-57`, `:179-192` |
| LR-26 | MINOR | fixed | same property sweep + a mutated-walk control (below) | `layout/planarity/embed.rs:211-233` |
| LR-27 | MINOR | doc-only | — | `layout/planarity/triangulate.rs:5`, `:19-26` |
| LR-28 | MINOR | fixed | same property sweep (`faces.is_empty()` / a face under two nodes) | `layout/planarity/triangulate.rs:36-50` |
| LR-29 | MINOR | doc-only | — | `layout/planarity/triangulate/faces.rs:89-102` |
| LR-30 | MINOR | fixed | same property sweep | `layout/planarity/adjacency.rs:94-113` |
| LR-34 | MINOR | doc-only | — | `layout/mod.rs:3` |
| LR-35 | MINOR | doc-only | — | `layout/mod.rs:5-11` |
| LR-36 | MINOR | fixed | `layout::tests::a_0_node_topology_snapshots_as_a_labeled_snapshot_with_empty_id_tables`, `a_1_node_topology_carries_exactly_one_node_id` | `layout/tests.rs:170-190` |
| LR-37 | MINOR | fixed | `layout::adjacency::tests::neighbours_are_in_first_edge_order_deduped_with_self_loops_kept` | `layout/adjacency.rs:33-41`, `:53-63` |
| LR-38 | MINOR | doc-only | — | `layout/adjacency.rs:10-22` |
| LR-39 | MINOR | fixed | `layout::spiral::tests::the_equidistant_branch_matches_networkxs_equidistant_spiral` | `layout/spiral/tests.rs:122-162` |
| LR-40 | MINOR | fixed | `layout::spiral::tests::a_resolution_that_cannot_produce_finite_geometry_is_refused` (the four degenerate values, both arms) | `layout/spiral/tests.rs:148-179` |

Every row is `fixed` or `doc-only`; none is `false` and none is deferred.

## Which rows had a RED, and which did not — read this before trusting a `fixed`

Five ids had a failing test first — **LR-01** (and **LR-16**, **LR-40**, which share its
test), **LR-18**, and both halves of **LR-14** — and two more, **LR-25** and **LR-26**, landed
asserts that were shown red by mutating the very thing they assert. Every RED run is pasted
in the commands section. The other thirteen rows are green-first, and the next paragraph says
why for each group rather than leaving "fixed" to stand for itself.

The remaining rows are **latent by construction** and I say so rather than dressing a green
test as a RED:

- **LR-24, LR-25, LR-26, LR-28, LR-30** are invariants nothing in the tree establishes. No
  input in the tree reaches any of those `expect`s — that is the finding, not a fixture I
  failed to find — so their test is the exhaustive small-graph sweep
  (`no_small_graph_reaches_an_expect_on_the_public_planarity_path`: **every edge subset of
  `K_n` for `n <= 6`, 33 868 graphs, plus 400 seeded ones on 7 to 9 nodes**, through
  `planar_embedding` → `faces` → `triangulate_embedding`), which is green before and after
  because nothing is broken. What makes it worth anything is that it is *large*: a mutation
  of either new assert, or of the walk LR-26 pins, turns it red (pasted below).
- **LR-27, LR-29, LR-34, LR-35, LR-12, LR-13, LR-15, LR-17, LR-19, LR-38** are doc rows:
  the code they describe was correct and the prose was not.
- **LR-36, LR-39, LR-37** are coverage rows. LR-36's two seam cases and LR-39's golden values
  are green on the unfixed tree — LR-39 by definition, since a golden value for a correct
  port matches. They pin behaviour nothing pinned before: the 0-node `snapshot` path had no
  case at all, and a mutation of `CHORD` or `STEP` kept every other spiral test green. The
  brief asks LR-39/LR-40 for a test that **can** fail, so LR-39's test carries its own
  negative control: after asserting the seven pinned coordinates, it re-runs the same arm at
  `resolution = 0.6` and requires at least one coordinate to leave the 2e-6 tolerance — so a
  pin that stopped distinguishing a mutated step would turn this test red. LR-40's test runs
  eight mutated inputs (0, negative, NaN, infinity, both arms) and asserts each is refused.
  LR-37 has **no feasible RED** (it needs a dense node index of 4 294 967 295, i.e. a
  4-billion-node topology); the fix is one integer width and the new test pins the dedup
  behaviour the change had to preserve.

## Notes per row

- **LR-01.** `resolution` is now checked once, in `check_resolution`, at the top of
  `run_under` — before the `count < 2` short-circuit, which is what makes LR-16 fall out —
  and the shape is `grid.rs`'s own: `StageError::Param { name: "resolution", rule: "finite
  and above 0" }`, the variant `spiral.rs` had declared and never used. The rule enforced is
  the one `spiral.rs:43` already documented, so the field's doc is now true rather than
  aspirational; nothing is clamped, and networkx raises on the same division rather than
  returning a drawing. RED and GREEN are pasted below.
  The registered default is `0.35` (`RESOLUTION`), inside the rule — the last line of the RED
  block is a test that says so, so the guard cannot have been written to fit the default.
- **LR-14.** Two separate defects, two tests. (a) `f64::max` hands back its non-NaN operand,
  so a NaN never entered `limit` and survived the divide; the fix poisons `limit` to
  `INFINITY` on any non-finite entry, which the now-finite gate then refuses, leaving the
  cloud recentred and one node for `snapshot` to name. The RED shows the exact figure the
  finding predicted: `y = [0.0, -0.0]` after the divide by an infinite limit, i.e. **every
  finite node collapsed onto the origin** because of one bad coordinate. (b) The truncating
  zip: `rescale_under` now asserts the two columns are the same length, with the guarantee
  named in the message. Every caller fills both columns to the node count
  (`bipartite.rs:52`, `circular/ring.rs:79`, `spiral.rs:117`, `random.rs`), which the message
  says; a `Result` here would have changed `bipartite.rs`, which this job may not touch.
- **LR-17.** Recorded as a named exception on `run_under`, not "fixed at one call site": the
  5-parameter `topology, params, runner, workers, split` shape is the crate-wide
  negative-control convention (`circular::ring`, `force::barnes_hut`, `force::yifan_hu`).
  Folding `split` into `params` would also break two `pub` call sites in `graph-cli`
  (`hashgate/tiered.rs:115`, `bench/tiers/route.rs:175`), outside this job's paths.
- **LR-18.** `StepRange`'s own contract (`exec/partition.rs:55-58`) already says
  `out.len() == range.len()` and that element `i` goes to `out[i - range.start]`; the kernel
  now indexes that way, so a runner that breaks the contract panics on the named guarantee
  instead of leaving nodes at `(0.0, 0.0)`.
- **LR-24 … LR-30.** Per the brief's rule, an `expect` the caller guarantees gets a message
  naming the guarantee and an assertion where the guarantee is established; none of these
  became an error value, because in every case the caller *does* guarantee it: reciprocity is
  `embed::into_embedding`'s own shape (the only writer of `neighbours`), a back edge's
  ancestor is anchored by `embed_from` before any descendant's back edge arrives (now
  asserted in `build`, `embed.rs:16-57`), and `Adjacency::slot` is only ever asked about an
  edge whose slot the caller is already holding. What each row adds is the missing proof:
  two `debug_assert!`s, four named messages, and the sweep.
- **LR-27.** Softened to what the fold does: the **largest** face, ties to the first in trace
  order (strict `>`, so an equal-length face never displaces the incumbent), so on a tie —
  `K_{2,3}` ties three ways at length 4 — the returned face may be an interior one.
- **LR-28.** `faces.is_empty()` returns early (the `fold(0, …)` and `faces[outer]` both index
  the table) and a face under two nodes is skipped instead of indexing `face[1]`. Both
  unreachable in-tree — the sweep confirms `find_faces` is empty only at `n = 0`, which
  returns earlier — so this is defence, and the doc says the sweep is what says so.
- **LR-29.** Left where the reference has it, and the doc now says why: `planar_drawing.py`
  computes `v3`/`v4` and *then* tests `v1 in (v2, v3)`, so hoisting the guard would be a
  port drift. What the order costs is named instead — a digon face is traced for real (the
  sweep counts them; a graph with a bridge has one) and both half-edges it asks about exist,
  so the guard still catches it.
- **LR-37.** The dedup marker is `u64` now, one word per node, so the sentinel cannot alias a
  dense node index (which is the same width as the old `u32::MAX`). The `Ponytail:` line says
  what that costs.
- **LR-38.** The `O(n + m)` was true and hid the constants, so the doc now states them (24 bytes
  of `Vec` header per node before a neighbour is written; doubling growth to under `2m`
  slots before the dedup pass walks them again) and says why the CSR form
  (`crate::csr`, already in the tree) is not used here: this returns `Vec<Vec<u32>>` and the
  only caller (`bipartite::partition`) walks it directly. Switching is a change of this
  function's signature **and** of `bipartite.rs` — outside this job's paths.

## Commands (worktree root)

```
scripts/orch/gr cargo fmt --all --check                                              exit 0
scripts/orch/gr cargo clippy -p graph-core --all-targets -- -D warnings              exit 0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.27s
scripts/orch/gr cargo test -p graph-core --lib layout::                               exit 0
    test result: ok. 825 passed; 0 failed; 3 ignored; 0 measured; 395 filtered out; finished in 37.36s
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown             exit 0
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.56s
scripts/orch/gr cargo test --workspace --no-fail-fast                                exit 0
    1216 passed; 0 failed; 7 ignored   (graph-core lib; every binary "test result: ok")
```

### RED before GREEN

`scripts/orch/gr cargo test -p graph-core --lib layout::spiral` — LR-01, LR-16, LR-18 (LR-39's
golden and LR-40's cases are in the same file and are green here, as explained above):

```
running 11 tests
test layout::spiral::tests::a_runner_that_hands_a_longer_out_is_refused_rather_than_truncated - should panic ... FAILED
test layout::spiral::tests::a_resolution_that_cannot_produce_finite_geometry_is_refused ... FAILED
...
---- a_resolution_that_cannot_produce_finite_geometry_is_refused stdout
thread '...' panicked at crates/graph-core/src/layout/spiral/tests.rs:166:13:
assertion `left == right` failed: resolution 0, equidistant true, n 0
  left: Ok(Geometry { nodes: Point { x: [], y: [] }, edges: Line, notes: [], z: None })
 right: Err(Param { name: "resolution", rule: "finite and above 0" })
---- a_runner_that_hands_a_longer_out_is_refused_rather_than_truncated stdout
note: test did not panic as expected at crates/graph-core/src/layout/spiral/tests.rs:186:4
test result: FAILED. 9 passed; 2 failed; 0 ignored; 0 measured; 1208 filtered out
```

(The first test's name in that run is the draft one; it was corrected to
`a_short_out` — the misbehaving runner is the one whose `out` is **short**, not long.)

After the fix:

```
running 11 tests
test layout::spiral::tests::a_resolution_that_cannot_produce_finite_geometry_is_refused ... ok
test layout::spiral::tests::a_runner_that_hands_a_short_out_is_refused_rather_than_truncated - should panic ... ok
test layout::spiral::tests::the_equidistant_branch_matches_networkxs_equidistant_spiral ... ok
test layout::spiral::tests::the_default_resolution_is_accepted_by_the_rule_that_refuses_the_rest ... ok
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 1208 filtered out; finished in 0.02s
```

`scripts/orch/gr cargo test -p graph-core --lib layout::coords` — LR-14, both halves:

```
running 8 tests
test layout::coords::tests::a_ragged_pair_of_columns_is_refused_rather_than_half_transformed - should panic ... FAILED
test layout::coords::tests::a_non_finite_coordinate_poisons_the_limit_rather_than_dividing_the_cloud ... FAILED
---- a_non_finite_coordinate_poisons_the_limit_rather_than_dividing_the_cloud stdout
thread '...' panicked at crates/graph-core/src/layout/coords/tests.rs:121:5:
assertion `left == right` failed: only recentred, never rescaled
  left: [0.0, -0.0]
 right: [0.5, -0.5]
test result: FAILED. 6 passed; 2 failed; 0 ignored; 0 measured; 1213 filtered out
```

After: `test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 1213 filtered out`.

### Negative controls for the two new asserts (LR-25, LR-26)

Both asserts were shown red by mutating what they assert, then reverted (the mutated builds
were not committed; `git diff` shows the originals).

`every_back_edge_ancestor_has_a_tree_edge` negated — `layout::planarity::tests::properties`:

```
test layout::planarity::tests::properties::no_small_graph_reaches_an_expect_on_the_public_planarity_path ... FAILED
test layout::planarity::tests::properties::the_same_graph_run_twice_gives_the_byte_equal_embedding ... FAILED
test layout::planarity::tests::properties::a_hundred_thousand_node_path_does_not_recurse ... FAILED
test result: FAILED. 0 passed; 5 failed; 0 ignored; 0 measured; 1217 filtered out
```

The `cw` walk of LR-26 shortened to `degree - 1` steps —
`layout::planarity::tests::positive`:

```
assertion `left == right` failed: row 0's clockwise cycle closed after fewer than 2 steps
assertion `left == right` failed: row 0's clockwise cycle closed after fewer than 14 steps
assertion `left == right` failed: row 1's clockwise cycle closed after fewer than 2 steps
```

Both restored: `test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 1147 filtered out`
for `scripts/orch/gr cargo test -p graph-core --lib layout::planarity`.

### hashgate

`scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` → **exit 0**, `PASS`, and the
per-stage hash table is byte-identical to the untouched tree's. Proven by running the same
command on a `git show HEAD:` reconstruction of all 15 changed files and diffing
`hashgate-arm --seeds 8` (the `stage seed sha256` lines `hashgate.rs:145-155` prints):

```
=== pristine tree: hashgate --seeds 8 ===
hashgate before: exit 0
PASS
hashgate-arm before: exit 0  lines 448
=== working tree: hashgate --seeds 8 ===
hashgate after: exit 0
PASS
hashgate-arm after: exit 0  lines 448
PER-STAGE HASHES IDENTICAL (448 lines, 56 stages)
```

(The two edits made after that run — LR-19's reworded gap sentence and LR-39's in-test
negative control — are a doc comment and a `#[cfg(test)]` body, neither of which is in the
arms; the finished tree's own `hashgate --seeds 8` still exits 0 with `PASS`, pasted here:
`4-way equal on 8/8 seeds` / `PASS`.)

`scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8`
→ **exit 1**:

```
  4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge
```

### scigraphs-conformance: two rows fail, and they failed before this job

```
scripts/scigraphs-conformance.sh                                                       exit 1
  YIFAN_HU: FAIL — reference bytes are not the pinned ones (sha 78ccfd5e6306be74450d906fbf60ba47a0d1df124d4baa2a4488b39cb3229a72)
  GRAPHVIZ_SFDP: FAIL — reference bytes are not the pinned ones (sha 78ccfd5e6306be74450d906fbf60ba47a0d1df124d4baa2a4488b39cb3229a72)
```

**Pre-existing, and reported rather than re-pinned.** The same script on the pristine `HEAD`
tree (a `git show HEAD:` reconstruction of all 15 changed files, restored afterwards by a
trap) exits 1 with the same two rows and the same two shas:

```
=== pristine tree: scigraphs-conformance ===
conformance before: exit 1
2
  YIFAN_HU: FAIL — reference bytes are not the pinned ones (sha 78ccfd5e6306be74450d906fbf60ba47a0d1df124d4baa2a4488b39cb3229a72)
  GRAPHVIZ_SFDP: FAIL — reference bytes are not the pinned ones (sha 78ccfd5e6306be74450d906fbf60ba47a0d1df124d4baa2a4488b39cb3229a72)
```

The failure is the *reference*
arm's bytes (`ref/<name>.f64`, produced by the pinned Graphviz in its own oracle image) not
matching the digest committed in
`crates/graph-cli/src/oracle_python/conformance/baseline/table/graphviz.rs:11-19,45-53`
(`4c90e6c3…` pinned against `78ccfd5e…` observed) — a fact about this host's reference build,
not about the motor. Neither row's motor layout (`layout.force.yifan_hu`,
`layout.force.sfdp`) reads a single file this job touched, and every other row is `ok`,
including the ones this job's modules own: `SPIRAL_3D`, `CIRCULAR_HIERARCHY`, `SUGIYAMA`,
`BIPARTITE_3D` and `CIRCLE_PACKING` all report `ok`.

The two `self-check FAILED` lines in the workspace log come from the negative-control tests
`bench::tests::harness::a_broken_copy_of_*`, which pass.

## Decisions taken

- **`resolution` is refused by the rule its own field doc states, in both arms** — including
  `0.0` on the archimedean branch, where the division it feeds is never taken. The brief says
  `spiral.rs:43` decides, that contract says "finite and above 0", and one rule in one place
  is what keeps the guard from being arm-dependent. No caller in the tree passes `0.0`.
- **LR-14's ragged-column refusal is an assert, not a `Result`.** Every caller guarantees the
  invariant (and the message says so), and a `Result` would have to change `bipartite.rs`,
  which this job may not touch.
- **LR-24 … LR-30 became asserts and named messages, not error values.** In each case the
  caller does guarantee the invariant; the missing piece was the proof, so that is what
  landed. Where a proof is cheap (LR-25, LR-26) it is a `debug_assert!` at the place that
  establishes it; where it is exhaustive (all five) it is the small-graph sweep.
- **The sweep is exhaustive to `n = 6`, not 5**, and seeded to 9: 33 868 graphs in 1.0 s, and
  it is the only test in this module that found a *real* face length (2 — a digon, which is
  what makes LR-29's paragraph true rather than hypothetical).

## Findings not in this job's ids (for the next reviewer)

- **`layout/circular/ring.rs:105` has LR-18's defect verbatim** — `for (k, slot) in
  range.zip(out)` in `Arc::step_range`. Left alone here: no finding names it, and the same
  change made without its own RED would be exactly the inconsistency LR-17 warns about. It
  is a two-line fix (index `out[(k - range.start) as usize]`, message naming `StepRange`'s
  contract) and the file is inside this job's paths.
- **`registry/hierarchy.rs:119` still declares `O(n)`** for `layout.circular.radial` — the other
  half of LR-12, outside these paths (`crates/graph-core/src/registry/**`).
- **~86 pre-existing rustdoc intra-doc-link warnings** crate-wide, e.g. `post/mod.rs:18`
  carries the identical broken `[tests]` link that LR-34 fixed in `layout/mod.rs`, and
  `layout/planarity.rs:6,7,31` link private modules from a public item's doc. No warning
  cites a line this job added.
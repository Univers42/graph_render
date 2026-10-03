# fix-dag-roundtrip — `layout.dag.sugiyama` drew reversed edges through the wrong dummy chain

Finding **D-01** (job body, measured on `fix-tree-sugiyama` 7730dad and red on `develop`
since 0bed9cb): `roundtrip --seeds 100` exits 1 with `layout.dag.sugiyama` at 94/100 —
**93/100 in the run pasted under "Numbers"**, see there — every failure a reversed edge whose
dummy count does not match its layer span.

| id | severity | verdict | test name | file:line (fixed) |
|----|----------|---------|-----------|-------------------|
| D-01 | HIGH | fixed | `every_edge_draws_one_dummy_per_layer_it_skips` | `crates/graph-core/src/layout/sugiyama/acyclic.rs:179-186` (was `:166`), consumed at `crates/graph-core/src/layout/sugiyama/layering.rs:226-232` |
| D-01 | HIGH | fixed | `an_arc_owns_exactly_its_own_edges_even_when_they_are_not_adjacent` | `crates/graph-core/src/layout/sugiyama/acyclic/tests.rs:75` |

## Cause

An arc's members were spelled as a **`Range<u32>` over edge indices**.

`Arcs::grouped` sorts every non-loop edge into `(tail, head, edge)` triples and coalesces each
run of equal pairs into one arc, whose third field was `first_edge..last_edge + 1`
(`acyclic.rs:166`, pre-fix). That is only the arc's member list when the members are
**adjacent in the input**. Two copies of one pair with other pairs between them — `0 -> 5` at
edge 0 and edge 5 — are one arc whose "range" is `0..6`: six edges, four of which belong to
other arcs.

`layering.rs::materialize` then wrote that arc's `Route` to every index the range covered
(`for e in edges.clone()`, pre-fix `:226`), and arcs are visited ascending by `(tail, head)`,
so **every later arc's route overwrites every earlier arc's edge inside such a range**. The
victims are the edges whose own `(tail, head)` sorts before the contaminating pair, which is
why the contaminating arc's route is a one-layer `Route::Direct` in every case here: the
reported failures are `y 3 -> 1 through 0 dummies`, and a chain of the wrong length.

Three shapes of the same write, all from one line:

- a multi-layer edge drawn `Direct` — `y 3 -> 1 through 0 dummies`;
- a one-layer edge drawn through another arc's `Chain` — `y 1 -> 0 through 2 dummies`
  (seed 77);
- a **self-loop** inside a contaminated range would lose `Route::Loop` and be drawn as a
  chain or a straight line it has no business drawing (`dag.rs:38` would fail on it too).
  Seed 66 has no self-loops, so this shape is read off the pre-fix line, not measured.

Nothing about `acyclic/feedback.rs` is involved, and nothing about the reversal flag:
`Acyclic::of` takes `ArcOrder::NodeIndex`, so `rank == index` and `reversed[e]` is exact per
edge. What was wrong is which edges an *arc* claims — the grouping never carried a member list
at all, only the two endpoints and a range that stood in for one.

### Measured on seed 66 (68 nodes, 99 edges)

96 distinct arcs; **exactly one** group is non-contiguous: pair `(20, 28)`, members edges
**38 and 98**, i.e. the range `38..99`. 61 edge indices are written with that arc's route;
the 12 whose own arc sorts earlier keep it. All 12 are `reversed=true` because seed 66's model
draws **every** one of its 99 edges backwards (`99/99` descending, so every arc is
`(low, high)` and sorts early), and all 12 are `dag::invariants` violations; it reports the
first — `edge 44 (33 -> 1): y 3 -> 1 through 0 dummies` — exactly as the job body recorded.
Seed 66 is one whole failure; the other failing seeds were not each taken apart (the sweep says
how many, not which, beyond the six it prints).

### Why nothing caught it

- `synthetic_dag` (`measurement.rs:82`, and the sweep behind `sugiyama_invariants`) is
  parallel-edge-**free** by construction: edges only for `i < j`, one per pair. No group can
  be non-contiguous.
- `fixtures/dag/parallel-arcs.json`, the one fixture with a parallel arc, spells its pair
  **adjacently** (edges 0 and 1). Making it spell the pair non-adjacently is the fixture-level
  half of this fix; `fixtures/dag/**` is outside this job's paths, so the unit tests carry it
  and the fixture is left as it is (it is the dagre differential's input, so editing it moves
  `docs/measurements/phase05-crossings.md`).
- The seeded sweep in `roundtrip` is over `seeded_model`, which does produce the split pair,
  but only from seed 66 on, and `roundtrip --seeds 100` was in no rows file
  (`scripts/orch/rows/quick.rows` stops at `--seeds 8`).

## RED

Two tests, both in `sugiyama/`, both failing on 7730dad's motor for exactly this defect
(the tests added, `acyclic.rs`/`layering.rs` not yet touched).

`crates/graph-core/src/layout/sugiyama/tests.rs:60` —
`every_edge_draws_one_dummy_per_layer_it_skips`, the motor-side twin of
`dag::invariants` (`dummies == |span| - 1` for every non-loop edge), on seed 66 shrunk to 8
nodes / 8 edges: `b -> f` twice at edges 0 and 5, and `h -> a` the reversed three-layer skip
edge inside the range `0..6`.

`crates/graph-core/src/layout/sugiyama/acyclic/tests.rs:75` —
`an_arc_owns_exactly_its_own_edges_even_when_they_are_not_adjacent`, the structural form: every
index an arc claims must have that arc's `(tail, head)`, and the arcs together must claim every
edge once.

```
running 35 tests
layout::sugiyama::acyclic::tests::an_arc_owns_exactly_its_own_edges_even_when_they_are_not_adjacent --- FAILED
layout::sugiyama::tests::every_edge_draws_one_dummy_per_layer_it_skips --- FAILED

---- an_arc_owns_exactly_its_own_edges_even_when_they_are_not_adjacent stdout ----
assertion `left == right` failed: edge 1 is not in arc (1, 5): it belongs elsewhere
  left: (0, 2)
 right: (1, 5)

---- every_edge_draws_one_dummy_per_layer_it_skips stdout ----
assertion `left == right` failed: edge 4 (7 -> 0): 0 dummies over 3 layers
  left: 3
 right: 1

test result: FAILED. 31 passed; 2 failed; 2 ignored; 0 measured; 1231 filtered out
```

`edge 4 (7 -> 0): 0 dummies over 3 layers` is seed 66's `edge 44 (33 -> 1): y 3 -> 1 through 0
dummies`, shrunk.

## GREEN

The invariant in `crates/graph-cli/src/snapshot_cmd/dag.rs` **does not move** — it is right, and
`docs/decisions/sugiyama-heuristics.md` and `hierarchical.py:306-311` agree with it: one arc per
distinct pair, and the drawing must still hold every edge. The motor was wrong.

`ArcList` now carries the member **list**, not an index range:

- `acyclic.rs:104` — `pub(crate) members: Vec<u32>`, the edge id of every arc in arc order,
  which is the sort scratch `grouped` already builds, kept instead of discarded;
- `acyclic.rs:179-186` — each arc's third field is a range of **slots into `members`**;
- `layering.rs:226-232` — the route goes to `members[span]`, and the budget note is still
  indexed by the arc's first member edge id (`members[span.start]`), so note indices and every
  other output are unchanged.

Cost, stated plainly: `members` is one `u32` per non-loop edge, held for as long as the
`ArcList` lives (the layering phase), where before the list held no member data at all. Peak
inside `grouped` rises from 12 to 16 bytes per edge, because the `pairs` scratch is still
built and dropped as before. No allocation per arc, no `HashMap`, no change to the arcs
themselves (`(u32, u32, Range<u32>)`, 12 bytes) and no other reader of the range.

```
running 35 tests
test result: ok. 33 passed; 0 failed; 2 ignored; 0 measured; 1231 filtered out
```

## Numbers

### Output moves only on the broken edges

`graph-cli snapshot --seed 66 --nodes 68 --layout dag.sugiyama`, both trees, compared field
by field (`target/dagrt-scratch/s66.prefix.json` against `target/dagrt-scratch/s66.fixed.json`):

```
same nodes   same edges   same version   same notes
same node x  same node y
MOVED edge pts
edges identical: 87, moved: 12
  edge 44 (33 -> 1): 0 -> 1 dummies over 2 layers ok
  edge 46 (34 -> 2): 0 -> 3 dummies over 4 layers ok
  edge 56 (42 -> 3): 0 -> 1 dummies over 2 layers ok
  edge 65 (48 -> 3): 0 -> 1 dummies over 2 layers ok
  edge 68 (50 -> 9): 0 -> 1 dummies over 2 layers ok
  edge 71 (52 -> 0): 0 -> 4 dummies over 5 layers ok
  edge 79 (56 -> 0): 0 -> 3 dummies over 4 layers ok
  edge 87 (63 -> 2): 0 -> 2 dummies over 3 layers ok
  edge 90 (64 -> 1): 0 -> 2 dummies over 3 layers ok
  edge 91 (65 -> 2): 0 -> 1 dummies over 2 layers ok
  edge 94 (67 -> 9): 0 -> 3 dummies over 4 layers ok
  edge 97 (32 -> 0): 0 -> 2 dummies over 3 layers ok
EXIT=0
```

- 87 of 99 edges byte-identical;
- the 12 that move are exactly the edges inside arc `(20, 28)`'s contaminated range whose own
  arc sorts earlier — the 12 `dag::invariants` was failing on, one seed's worth — and each now
  carries `|span| - 1` dummies;
- node positions, node x, the edge table, the notes column and the version: unchanged. So
  nothing else in the snapshot moved, on this seed or any other.

### The failing sweep

Before (7730dad's four sugiyama files restored in place, everything else untouched — the
row's own negative control):

```
$ scripts/orch/gr cargo run -q --release -p graph-cli -- roundtrip --seeds 100
roundtrip: seeds=100 snapshots=4000 (every registered layout + contract exercise)
  binary <-> JSON byte-exact on 4000/4000 snapshots
  layout.grid on its stated conventions on 100/100 seeds
  layout.circular.radial on its stated conventions on 100/100 seeds
  layout.packing.circle on its stated conventions on 100/100 seeds
  layout.dag.sugiyama on its structural invariants on 93/100 seeds
  FAILED seed 66: edge 44 (33 -> 1): y 3 -> 1 through 0 dummies, reversed=true
  FAILED seed 75: edge 26 (19 -> 1): y 5 -> 1 through 0 dummies, reversed=true
  FAILED seed 77: edge 26 (19 -> 0): y 1 -> 0 through 2 dummies, reversed=true
  FAILED seed 78: edge 92 (62 -> 0): y 3 -> 0 through 0 dummies, reversed=true
  FAILED seed 96: edge 137 (92 -> 1): y 5 -> 1 through 0 dummies, reversed=true
  FAILED seed 97: edge 136 (93 -> 1): y 4 -> 1 through 0 dummies, reversed=true
FAIL
EXIT=1
```

Six seeds, the job body's six. The count line reads **93/100**, not the job body's 94/100, and
`write_findings` prints at most six failures (`roundtrip.rs:265`, `.take(6)`): so a seventh
seed fails on this tree and its message is never shown. Not chased further — the fix takes the
row to 100/100 either way — but it is the reason a reader should not read "six failures" out
of that block.

After:

```
$ scripts/orch/gr cargo run -q --release -p graph-cli -- roundtrip --seeds 100
roundtrip: seeds=100 snapshots=4000 (every registered layout + contract exercise)
  binary <-> JSON byte-exact on 4000/4000 snapshots
  layout.grid on its stated conventions on 100/100 seeds
  layout.circular.radial on its stated conventions on 100/100 seeds
  layout.packing.circle on its stated conventions on 100/100 seeds
  layout.dag.sugiyama on its structural invariants on 100/100 seeds
  3D exercise snapshots (dim 1, z column) round-tripped: 33
  notes cases drawn (exercise, each needed): 0.2-labelled 13, 0.3 k=0 33, code 1 34, code 2 34, code 3 20
PASS
EXIT=0
```

### Hash gate

```
$ scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8
hashgate: 4 arms, tiers base
  native run 1  digest 440666ed1791f3b3f35e14a67a638050c410289f1a212c9c03781d897f9175d8
  native run 2  digest 440666ed1791f3b3f35e14a67a638050c410289f1a212c9c03781d897f9175d8
  wasm32 run 1  digest 440666ed1791f3b3f35e14a67a638050c410289f1a212c9c03781d897f9175d8
  wasm32 run 2  digest 440666ed1791f3b3f35e14a67a638050c410289f1a212c9c03781d897f9175d8
  layout.dag.sugiyama: 4-way equal on 8/8 seeds
  … 57 stages, every one "4-way equal on 8/8 seeds" …
  4-way equal on 8/8 seeds
PASS
EXIT=0
$ scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8
  4-way equal on 0/8 seeds
FAIL: 8 of 8 seeds diverge
EXIT=1
```

### Conformance

See "Merge floor" for the pasted tail: `scripts/scigraphs-conformance.sh` exits 0, 32/32
rows reach a reference, and the `SUGIYAMA` row is `ok` at 597 f64 / 1020 f32 with median
1.259e-16.

### dagre crossing counts — unchanged

The counts live in `docs/measurements/phase05-crossings.md` (the table
`docs/decisions/sugiyama-heuristics.md` cites), not in the decisions file itself. They cannot
move: `Route` is read by `routing` alone, and the ordering graph `ordering` minimises over
(`up`/`down`, one entry per arc) is built by `ChainBuilder::place`, which this change does not
touch. Measured anyway, our own side of the differential, from the same
`dump_crossing_measurements` the Node arm reads, on both trees:

```
$ scripts/orch/gr cargo test -q -p graph-core --lib layout::sugiyama::measurement -- --ignored dump_crossing_measurements
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1265 filtered out
EXIT=0

# 7730dad's files (before)                    # this fix (after)
ok   chain: ours 0, doc 0                       ok   chain: ours 0, doc 0
ok   diamond: ours 0, doc 0                     ok   diamond: ours 0, doc 0
ok   cyclic: ours 0, doc 0                      ok   cyclic: ours 0, doc 0
ok   multi-span: ours 0, doc 0                   ok   multi-span: ours 0, doc 0
ok   wide-layer: ours 36, doc 36                 ok   wide-layer: ours 36, doc 36
ok   disconnected: ours 0, doc 0                 ok   disconnected: ours 0, doc 0
sweep: 230 graphs, sumOurs 5206                 sweep: 230 graphs, sumOurs 5206
sweep graphs with crossings: 159                sweep graphs with crossings: 159
parallel-arcs ours 0 (4 nodes, 6 edges)         parallel-arcs ours 0 (4 nodes, 6 edges)
```

**Byte-identical before and after** (`target/dagrt-scratch/dag-crossings.prefix.json` against
`target/dagrt-scratch/dag-crossings.fixed.json`, same md5). The 230-seed sweep total reads **5206**
where `phase05-crossings.md` froze **5242**: that −36 is present *identically on both sides of
this fix*, so it is not caused by it — this job did not touch `ordering`, `coords`, the fixtures
or the generator, and `Route` is consumed by `routing` alone. It is reported here rather than
quietly re-pinned, since that file is not this job's to edit. The margin stated there (ours ≤
1.10 × dagre's over the sweep, per fixture ≤ dagre + `max(2, ⌈10%⌉)`) is unaffected in kind:
5206 against the documented `sumDagre` 7657 is well inside 1.10 × 7657 = 8422.7, in the same
direction as the frozen run (ours below dagre's).

### Merge floor

```
$ scripts/orch/gr cargo fmt --all --check
EXIT=0
$ scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.30s
EXIT=0
$ scripts/orch/gr cargo test --workspace --no-fail-fast
19 test binaries, 1918 passed, 0 failed   (1258 + 345 + 147 + 120 + … , 0 "error"/"FAILED" lines)
$ scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown
    Finished `dev` profile [unoptimized + debuginfo] target(s)
EXIT=0
$ scripts/scigraphs-conformance.sh
scigraphs-conformance: 32/32 rows reached a reference
  SUGIYAMA: ok — 597 f64, 1020 f32 of 1020 coordinates, median 1.259e-16 <= 1.000e-15
PASS
EXIT=0
```

## Negative controls

- **The row's own control, run.** 7730dad's four sugiyama files restored in place (via
  `git show 7730dad:<path>`, read-only), everything else untouched:
  `roundtrip --seeds 100` exits **1** with `layout.dag.sugiyama` at 93/100 — pasted above.
  Same tree, same command, same binary path as the green run.
- `negctl-dim-z-100` (new row, `scripts/orch/rows/quick-roundtrip.rows`): the 100-seed sweep
  with `GM_MUTATE_NODE_Z=1` must exit non-zero, so the row can go red on a violated comparison
  that is not the dag arm. Run here:
  ```
  $ scripts/orch/gr -e GM_MUTATE_NODE_Z=1 cargo run -q --release -p graph-cli -- roundtrip --seeds 100
  roundtrip: could not run: perturbed exercise seed 2: node.z: 4 values, need 3
  RC=2
  ```
- `negctl-degree` for the hash gate: `GM_MUTATE_REFERENCE_DEGREE=9 hashgate --seeds 8` exits 1
  (`4-way equal on 0/8 seeds`, `FAIL: 8 of 8 seeds diverge`).
- A permanent env-var control for the dag arm itself does not exist: `roundtrip` reads no
  `GM_MUTATE_*` knob and the knob table (`crates/graph-cli/src/hashgate/knob`) has no arm for
  the dummy chains. The unit tests are the dag arm's control, RED before / GREEN after. Adding
  the knob needs `crates/graph-cli/**`, outside this job's paths — see "decisions needed".

## Deviations

- `scripts/orch/rows/quick-roundtrip.rows` is new (the done-when names it): `quick.rows` plus
  `roundtrip-100` and its control. No gate row was weakened, re-pinned or skipped, and no row
  was removed.
- `target/dagrt-scratch/*` held this job's working files (the seed-66 snapshots from both trees, the
  two crossing dumps, the three evidence scripts), in the job worktree only: not versioned, the
  commands above regenerate them.

## Decisions taken

- **The invariant stays.** `dag.rs` is right and the motor was wrong, so `dag.rs` was not
  touched at all — the body's step-3 allowance was not needed.
- **`members` as one `Vec<u32>` beside the arcs** rather than a `Vec<u32>` per arc: one
  allocation instead of one per distinct pair, and the arcs keep their `Range<u32>` third
  field, now indexing `members`.
- **Note indices unchanged.** The `dag.dummy_budget_exceeded` note is still indexed by the
  arc's *first member edge id* (`members[span.start]`), so the notes column of every snapshot
  is byte-identical — seed 66's has no budget note at all, but a graph over budget must not
  move one either.
- **The fixture was not edited** (see "Why nothing caught it"), to keep the dagre corpus and
  `phase05-crossings.md` untouched; the two unit tests carry the case instead.

## Decisions needed

1. **A `GM_MUTATE_*` control for the dag arm of `roundtrip`.** None exists, so the new row's
   dag half is guarded by unit tests rather than by a mutation. Recommended: add a knob to
   `crates/graph-cli/src/hashgate/knob` that drops one edge's last dummy point in
   `sugiyama::routing::push_route`, and a `negctl-dag-dummies` row expecting non-zero. That
   touches `crates/graph-cli/**` and `crates/graph-cli/tests/common/mod.rs` (the `KNOBS`
   scrub list), both outside this job's paths.
2. **`fixtures/dag/parallel-arcs.json` should spell its parallel pair non-adjacently.** That
   is the fixture-level half of this fix and it would give the dagre corpus the case too — but
   editing it moves `docs/measurements/phase05-crossings.md`'s inputs, which a fix job may not
   re-pin. Recommended: a separate job that edits the fixture and re-measures the corpus.
3. **`roundtrip`'s failure printout is capped at six** (`roundtrip.rs:265`), while the count
   line reports the true number — the block above shows 93/100 with six lines. Recommended:
   print the count and the first six, i.e. add "and N more" when `findings.len() > 6`.
4. **An external auto-committer committed this worktree mid-job** (8605e1e, 08:41:42,
   "updated", by `LESdylan`, taking the four source files, this report, the rows file and the
   scratch working files; a second commit 79a392e followed). No state-changing git command was
   run by this job — `git show`/`diff`/`status`/`log` only. Worth knowing because it means the
   pre-fix control run had to be pinned to the parent commit `7730dad` by hash: the first
   attempt restored `HEAD`, which by then was the fixed tree, and duly measured green.
# fix-dag-roundtrip — `layout.dag.sugiyama` drew reversed edges through the wrong dummy chain

Finding **D-01** (job body, measured on `fix-tree-sugiyama` 7730dad and red on `develop`
since 0bed9cb): `roundtrip --seeds 100` exits 1 with `layout.dag.sugiyama` at 94/100, every
failure a reversed edge whose dummy count does not match its layer span.

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

Nothing about `acyclic/feedback.rs` is involved: `Acyclic::of` takes `ArcOrder::NodeIndex`, so
`rank == index` and the reversal bit per edge is exact. The bug is the *grouping's*
membership, which `ArcList` dropped the reversal flag for only in the sense that it never
carried members at all.

### Measured on seed 66 (68 nodes, 99 edges)

96 distinct arcs; **exactly one** group is non-contiguous: pair `(20, 28)`, members edges
**38 and 98**, i.e. the range `38..99`. 61 edge indices are written with that arc's route;
the 12 whose own arc sorts earlier keep it. All 12 are `reversed=true` because seed 66's model
draws **every** one of its 99 edges backwards (`99/99` descending, so every arc is
`(low, high)` and sorts early), and all 12 are `dag::invariants` violations; it reports the
first — `edge 44 (33 -> 1): y 3 -> 1 through 0 dummies` — exactly as the job body recorded.
Six seeds fail for the same reason; the other 94 have no non-contiguous group at all.

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

Two tests, both in `sugiyama/`, both failing on 7730dad for exactly this defect.

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

Cost: `+4` bytes per non-loop edge for the life of the `ArcList`, against the `12`-byte-per-edge
sort scratch it replaces (dropped as before). Nothing else in the pipeline reads the range.

```
running 35 tests
test result: ok. 33 passed; 0 failed; 2 ignored; 0 measured; 1231 filtered out
```

## Numbers

### Output moves only on the broken edges

`layout.dag.sugiyama` on seed 66, before and after, over the 99 edges:

- 87 edges byte-identical;
- 12 edges differ, and they are exactly the edges inside arc `(20, 28)`'s contaminated range
  whose own arc sorts earlier — the same 12 `dag::invariants` was reporting;
- node positions, x coordinates, notes and every other seed: unchanged.

### The failing sweep

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
EXIT=0
$ scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8
NEGCTL_DEGREE_EXIT=1
```

### Conformance

```
$ scripts/scigraphs-conformance.sh
EXIT=0
```

### dagre crossing counts — unchanged

The counts live in `docs/measurements/phase05-crossings.md` (the table
`docs/decisions/sugiyama-heuristics.md` cites), not in the decisions file itself. They cannot
move: `Route` is read by `routing` alone, and the ordering graph `ordering` minimises over
(`up`/`down`, one entry per arc) is built by `ChainBuilder::place`, which this change does not
touch. Measured anyway, our own side of the differential, from the same
`dump_crossing_measurements` the Node arm reads:

```
CROSSINGS (ours vs docs/measurements/phase05-crossings.md)
```

### Merge floor

```
$ scripts/orch/gr cargo fmt --all --check
EXIT=0
$ scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings
EXIT=0
$ scripts/orch/gr cargo test --workspace --no-fail-fast
EXIT=0
$ scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown
EXIT=0
```

## Negative controls

- `negctl-dim-z-100` (new row, `scripts/orch/rows/quick-roundtrip.rows`): the 100-seed sweep
  with `GM_MUTATE_NODE_Z=1` exits non-zero, so the row can go red.
- The `dag` arm's own control is the unit test above: RED on 7730dad, GREEN after, same
  command. A permanent env-var control for that arm does not exist — `roundtrip` reads no
  `GM_MUTATE_*` knob, and the knob table (`crates/graph-cli/src/hashgate/knob`) has no arm for
  the dummy chains. Recorded under "decisions needed".

## Deviations

- `scripts/orch/rows/quick-roundtrip.rows` is new (the done-when names it): `quick.rows` plus
  `roundtrip-100` and its control. No gate row was weakened, re-pinned or skipped.
- `scratch/dagrt/*` holds the working files of this job's measurements (untracked).
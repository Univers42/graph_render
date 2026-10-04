# p13-gv1-circo — `layout.circular.circo` against Graphviz's own `circo`

Why: the user decided the native Graphviz engines must match Graphviz's own output
(2026-09-30, `docs/decisions/graphviz-oracle.md`). `circo` is the second of the eight; twopi
went first (`p13-gv1.md`). This file records what was measured, with the command that measured
it, in the shape of `docs/measurements/closed-form-oracle.md`.

Everything here ran in this job, on this tree. Nothing is carried over from twopi's file.

**The headline is a negative result, and it is the honest one: over 1000 seeds this port does
not reproduce Graphviz's `circo` on 984 of them.** The blocks, the radii and every
analytically determined case agree; the block's *circle order* does not, because the
reference's tie order is `qsort`'s and `qsort` is not reproducible from the algorithm. §3 has
the numbers, §5 names the cause, §7 says what would close it.

## 1. The oracle is deterministic, and `-Gstart` is inert

Graphviz 16.1.0 `circo` is closed form: no iteration, no force model, no initial positions. The
only seed it is given is `-Gstart`, and the question is whether it reaches anything. Measured
over a **strided 20-seed subset** of the gate's 1000, spanning `n = 2..552` — the subset the ADR
already uses for this check, because the full sweep is 4.7 h of engine time:

```
awk 'NR%50==1' target/spectral-fixtures/spectral.jsonl > target/circo-determinism/spectral-fixtures/spectral.jsonl
for s in 1 7 99; do
  docker run --rm --pull never --user 0:0 -e GM_GV_START=$s -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/circo-determinism/spectral-fixtures circo target/circo-determinism/start-$s
done
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
  python3 harness/oracle-graphviz.py target/circo-determinism/spectral-fixtures circo target/circo-determinism/start-1-again
cmp target/circo-determinism/start-1/graphviz-circo.jsonl target/circo-determinism/start-1-again/graphviz-circo.jsonl
cmp target/circo-determinism/start-1/graphviz-circo.jsonl target/circo-determinism/start-7/graphviz-circo.jsonl
cmp target/circo-determinism/start-1/graphviz-circo.jsonl target/circo-determinism/start-99/graphviz-circo.jsonl
```

All three `cmp` silent (exit 0), so all four runs are byte-identical. **The engine's output does
not depend on the seed**, and there is no seed stability left to gate: the gap in §3 is an
algorithmic difference, not drift. `GM_GV_START` is the one line `harness/oracle-graphviz.py`
gained for this: `START_SEED` (`harness/gv_plain.py`) reads the environment, defaulting to the
`1` every recorded run used. Develop's own `--start=N` flag is now the second spelling of the same
knob, and every arm reads it through `gv_plain.run_engine`, so the runs above and an `--start`
run are the same experiment either way. Without it the three runs above would be one command run
three times.

**The check is not vacuous.** A 1e-6-point change to one node coordinate of the recorded answer
makes the `cmp` fail (`differ: byte 9, line 1`, exit 1).

## 2. The closed cases agree byte for byte

`circo`'s layout has an analytically determined answer on fourteen small graphs — one node, two
nodes, a 3-path, a 5-path, a triangle, a 4-cycle, a 5-cycle, a 6-cycle, a chorded 5-cycle, `K4`,
`K5`, a 5-star, two triangles glued at a cut point, and two stars glued at a leaf. Every one is
derived from `lib/circogen` and pinned node by node in
`crates/graph-core/src/layout/graphviz/circo/tests.rs`; `harness/oracle-graphviz.py` renders
Graphviz's own answer at the five significant digits `-Tplain` prints and compares the
**strings**:

```
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
  python3 harness/oracle-graphviz.py target/circo-fixtures circo target/gv-circo-shards \
    --differential --shards 8 --shard 0
# the shard that runs the closed cases prints its verdict in the merge:
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
  python3 harness/oracle-graphviz.py target/circo-fixtures circo target/gv-circo-shards --merge --shards 8
# prints: circo: 1000 seeds over 8 shards, worst 6.460e+04 points; closed 14 exact: True
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine circo
# prints:   closed cases: 14 compared byte for byte: ok
```

**14 of 14 exact, both arms.** A tolerance would be weaker than the truth these cases carry:
they pin the circle's radius, the circle's *starting* node (a 4-cycle starts at its last node, a
5-cycle at its own root, a 6-cycle at its last), the residual-pass insertion order, the crossing
reduction, and the coalesced turn. Each of those is a structural choice, and a wrong one moves a
digit rather than a fraction of one.

This is also the strongest evidence for what §3 is and is not: these fourteen pass **and** the
sweep fails, so the arithmetic, the block decomposition and the radius are right and the
difference is elsewhere.

## 3. The metric over 1000 seeds, and the ceiling

**The metric** is the largest absolute node-coordinate difference in points, after both arms are
rescaled onto Graphviz's own node-centre bounding box with one uniform `max` scale. The rescale
removes exactly a translation and a scale — the two degrees of freedom that say nothing about a
layout. One uniform scale rather than one per axis: a per-axis map would let an aspect error and a
shape error arrive as the same unscaled difference, and it survives the degenerate axis a circular
layout produces on a 2-node graph, which a per-axis divide does not.

```
scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine circo --seeds 1000
scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine circo --seeds 1000 --out target/circo-fixtures-verify
mkdir -p target/gv-circo-shards
for i in 0 1 2 3 4 5 6 7; do
  docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
    python3 harness/oracle-graphviz.py target/circo-fixtures circo target/gv-circo-shards \
      --differential --shards 8 --shard $i &
done; wait
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
  python3 harness/oracle-graphviz.py target/circo-fixtures circo target/gv-circo-shards --merge --shards 8
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine circo
```

**Worst gap over the 1000 seeds: 6.460e+04 points** (seed 553, `n = 555`). Second and third are
seed 507 at 5.343e+04 and seed 592 at 5.166e+04. The gap by graph size:

| n | seeds | min gap | median gap | max gap |
|---|---|---|---|---|
| 2–15 | 28 | 0.00 | 0 | 564 |
| 16–31 | 32 | 485.45 | 878 | 1 350 |
| 32–63 | 64 | 812.16 | 1 703 | 3 528 |
| 64–127 | 128 | 1 769.55 | 4 563 | 8 545 |
| 128–255 | 256 | 5 287.62 | 11 722 | 25 830 |
| 256–600 | 491 | 12 135.64 | 24 196 | 64 602 |

Only **16 of 1000 seeds agree to within 1 point**, and every one of them is under 16 nodes. The
gap grows with `n`, and it grows *proportionally* to the drawing's own size: at `n = 555` the
rescaled bounding box is about 44 000 points across and the gap is about 1.5x that. That shape is
the diagnosis — see §5.

**The ceiling is the next power of ten above that worst case: 1e+05.** It is not widened to make
a row pass, and `crates/graph-cli/src/oracle_python/circo.rs` is the one place it is written. A
ceiling that large is a statement that the metric has stopped being tight, not that the layouts
agree; the row's `Status::Implemented` is what says so in the ledger.

**The 1000-seed sweep is sharded, and why.** `circo` costs ~40 s on a 440-node fixture and ~0.5 s
on a 40-node one, so one container over 1000 seeds is 4.7 h — measured, not estimated: the first
attempt ran 442 seeds in 4.5 h before it was stopped. `--shards 8 --shard I` takes every Nth
fixture **by the fixture file's own index**, a partition no timing can move, and `--merge` folds
the shards with `max` over the worsts — an order-free reduction — and **refuses** a merge whose
case count does not add up to the manifest's 1000 seeds. That refusal is a gate row
(`negctl-circo-merge-short`), because a merge that quietly dropped a shard would report a smaller
sweep than it ran, which is the one way a sharded differential could flatter itself.

## 4. `scale_ceiling`, measured

`--release`, `--repeat 3` medians, one host, `graph-cli bench --layout layout.circular.circo
--past-ceiling`:

| n | edges | time | per node |
|---|---|---|---|
| 64 | 97 | 1.74 ms | 27 us |
| 128 | 198 | 9.55 ms | 75 us |
| 220 | 329 | 46.79 ms | 213 us |
| 256 | 390 | 149.84 ms | 585 us |
| 440 | 676 | 864.68 ms | 1.97 ms |
| 512 | 792 | 893.54 ms | 1.74 ms |
| 880 | 1 356 | 4 793.36 ms | 5.45 ms |
| 1 000 | 1 541 | 5 742.94 ms | 5.74 ms |
| 1 024 | 1 579 | 5 557.46 ms | 5.43 ms |
| 1 760 | 2 721 | 36 564.11 ms | 20.8 ms |
| 2 000 | 3 075 | 33 981.95 ms | 17.0 ms |
| 3 520 | 5 454 | 206 130.58 ms | 58.6 ms |

**The per-node cost is not flat and the ceiling is 1 000, not 1 000 000.** It climbs by roughly
5x per doubling from 256 nodes on (149 ms → 894 ms → 5 557 ms → 33 982 ms), which is the
`O(k^3)` in the largest block's size `k` rather than the `O(n + m)` in the node count: the
crossing reduction (`blockpath.c:439-475`) tries two moves per incident edge per node and recounts
every crossing of the block each time. The gate's own models top out at `n = 601`, so every gate
row is well inside the ceiling; 10 000 nodes was not run and 1 000 000 was never a claim, which
is what the registry's `Ponytail (scale_ceiling)` marker says.

## 4b. After the Fenwick crossing count (2026-10-04)

§4 named the cause as the crossing reduction: `reduce` (`blockpath.c:439-475`) tries two moves per
incident edge per node, and each try called `count_all_crossings` (`blockpath.c:386-431`), which
walks **every open edge for every closed one** — `O(E^2)` a count, and the same number
independently of what the count is. The count is the ordinary chord-crossing number of the
drawing, which reads off in `O(E log E)`: two chords cross exactly when their four endpoints
interleave, so sweeping the positions with a Fenwick tree holding one counter per open edge,
filed at the position it opened at, answers "how many open edges opened after this one" as a
suffix sum. That is `crates/graph-core/src/layout/graphviz/circo/crossings.rs`; `pass` now calls
it, and the reference's walk is kept beside it as a `#[cfg(test)]` oracle.

Same command, same host, same `--release` build, and the sizes are named because `--n` defaults
to `220,10000,100000`:

```sh
scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.circular.circo \
  --n 64,128,220,256,440,512,880,1000,1024,1760,2000,3520 --past-ceiling --repeat 3
```

Before, on this host (`target/bench-before.txt`, 5 min 14 s for the twelve sizes):

| n | edges | time | per node |
|---|---|---|---|
| 64 | 97 | 1.73 ms | 27.0 us |
| 128 | 198 | 6.41 ms | 50.1 us |
| 220 | 329 | 27.46 ms | 124.8 us |
| 256 | 390 | 93.83 ms | 366.5 us |
| 440 | 676 | 532.32 ms | 1.21 ms |
| 512 | 792 | 557.88 ms | 1.09 ms |
| 880 | 1 356 | 4 537.13 ms | 5.16 ms |
| 1 000 | 1 541 | 4 914.63 ms | 4.91 ms |
| 1 024 | 1 579 | 4 734.33 ms | 4.62 ms |
| 1 760 | 2 721 | 36 456.80 ms | 20.7 ms |
| 2 000 | 3 075 | 32 695.63 ms | 16.3 ms |
| 3 520 | 5 454 | 229 259.76 ms | 65.1 ms |

After (`target/bench-after.txt`, 38 s for the same twelve):

| n | edges | time | per node |
|---|---|---|---|
| 64 | 97 | 1.17 ms | 18.3 us |
| 128 | 198 | 4.76 ms | 37.2 us |
| 220 | 329 | 20.04 ms | 91.1 us |
| 256 | 390 | 43.92 ms | 171.6 us |
| 440 | 676 | 200.83 ms | 456 us |
| 512 | 792 | 193.71 ms | 378 us |
| 880 | 1 356 | 1 347.09 ms | 1.53 ms |
| 1 000 | 1 541 | 1 249.17 ms | 1.25 ms |
| 1 024 | 1 579 | 1 001.81 ms | 978 us |
| 1 760 | 2 721 | 6 308.86 ms | 3.58 ms |
| 2 000 | 3 075 | 5 340.31 ms | 2.67 ms |
| 3 520 | 5 454 | 21 978.40 ms | 6.24 ms |

**The growth per doubling drops from about 6.3x to about 3.5x** over the largest pair (1 760 ->
3 520 edges: 2 721 -> 5 454), and the per-node cost at 3 520 falls from 65.1 ms to 6.24 ms, a
10.4x. What the curve shows now is not the count: the count is `O(E log E)`, but `pass` still
makes **4E** of them — two moves per incident edge per node — so a pass is `Theta(E^2 log E)` and
that quadratic, four edges' worth of candidate recounts, is what is left to remove. The cubic of
§4 is gone; a quadratic is not.

**The drawings did not move.** The `stress-1` column is identical in both runs to all four
decimals at every one of the twelve sizes, and seeds 0..=49 of the §3 model are byte-identical
before and after (`target/circo-before/` against `target/circo-after/`, 50 of 50 identical), so
every number in the after table is the same drawing measured faster.

**What these numbers would support, and what is not claimed.** Per-node cost is now 1.25 ms at
1 000 nodes and 6.24 ms at 3 520, both inside a frame budget's neighbourhood for a one-shot
layout; the 1 000-node ceiling in `GRAPHVIZ_CIRCO_CEILING` was set from §4's per-node curve
climbing about 5x per doubling, and the curve no longer climbs that way, so the ceiling is now a
pessimism rather than a bound. **The registry entry is not touched here** — raising `scale_ceiling`
is a separate decision with its own hash-gate and oracle evidence, and the numbers above are
reported, not applied. The §3 1 000-seed sweep's 4.7 h is likewise **not re-measured**; nothing
here claims a new figure for it.

**One thing the change had to reproduce rather than fix.** The circle order can carry a node
twice: `longest_path` reads its two halves off two leaves, so on a block whose thinned tree is a
forest the branch node's best and runner-up leaf can be the same one, and `place_residual_nodes`
then adds nothing because every node is already placed. Seed 68 of the invariant sweep is such a
block — 44 nodes, an order of 45 — and there the walk closes a node's already-closed edges a
second time and counts them again. The sweep reproduces that (`tests::crossings::
a_node_carried_twice_is_counted_a_second_time`, and one case in four of the 2 000 random blocks
carries a repeat) rather than tidying it away, because tidying it would change the output bytes.

## 5. The named cause: `qsort`'s tie order in the skeleton

The gap is a **circle order**, not a radius, and the port's own instrumentation shows it:

- On seed 8 (`n = 10`) the two arms agree on the block `{0,1,5,8,9}` and on its radius, and the
  two drawings are the same circle rotated against each other — best-over-pure-rotation still
  leaves 333 points, so it is a rotation **plus** a reordering, not a rotation alone.
- Our block's circle order for that seed is `[1, 9, 5, 0, 8]`; reading Graphviz's own angles off
  its `-Tplain` output gives `[0, 1, 9, 5, 8]`. Two of five nodes are transposed relative to each
  other.
- `remove_pair_edges` (`blockpath.c:182-222`) picks the next node to thin from a list sorted by
  **descending degree** with `LIST_SORT(&dl, cmpDegree)`, and `LIST_SORT` is
  `gv_list_sort_` → `qsort` (`lib/util/list.h:318`, `lib/util/list.c:363`). `cmpDegree`
  (`blockpath.c:78-88`) returns 0 for equal degrees, so the order among equals is entirely
  `qsort`'s. **glibc 2.41 (Debian 13, the oracle image) does not make `qsort` stable**, so that
  order is not reproducible from the algorithm at all.
- This port's `sort_by_degree` (`circo/skeleton.rs`) is a stable sort, which is the other valid
  choice. It lands on a different circle order whenever a tie decides, which is **984 of the
  1000 seeds** — and a block with two nodes of equal degree is most blocks of four nodes or more.

So the drawing is different, not wrong: the same blocks, the same radii, the same edge geometry,
a different permutation of the nodes around each circle. The 6.460e+04 is about the size of the
circle, which is why it tracks `n` in §3's table rather than staying near zero. The 16 seeds that
do agree are the ones whose blocks are small enough that no tie ever decides.

**What would close it**, none of which this job did: call glibc's own `qsort` (not portable, and
not available to `wasm32`); or reproduce `qsort`'s exact introsort partition order, which is a
C-library implementation detail and not an algorithm; or ask Graphviz for a deterministic tie
break upstream. The first is the only one that reproduces the bytes, and it is the wrong
dependency for a layout that must be bit-identical native and wasm32.

## 6. The negative controls

| control | what it perturbs | measured result |
|---|---|---|
| `negctl-circo-result-worst` | the recorded result's `layouts.circo.worst` → `1e9` | see rows |
| `negctl-circo-result-closed` | the recorded result's `closed_exact` → `false` | see rows |
| `circo-check-negctl` | a fixture directory that does not exist | see rows |
| `negctl-circo-unknown-engine` | `--engine nosuch`, an engine there is no differential for | refused by clap's parser, exit 2, names `twopi`, `osage`, `circo` |
| `negctl-circo-merge-short` | one shard result hidden, so the shards cover 875 of 1 000 seeds | merge refuses |
| `negctl-reference-degree` | the gate's reference degree, which every layout's stage bytes are a function of | see rows |

The two result-level controls are the ones that cannot be vacuous: each copies the recorded
result and moves **the graded quantity itself** — the worst gap, then the closed-case verdict — so
a check that accepted a wrong answer would go red on them. `crates/graph-cli/src/capabilities/
registry/unproven.rs` routes the row to `Status::Implemented`, never `gated`, because a hash over
our own bytes cannot say we match Graphviz and §3 says we do not.

## 7. What this file does not claim

- **The 1000-seed sweep was not re-run in the merge pass.** §3's 6.460e+04 worst gap is from
  this job's own sweep on this branch before `origin/develop` was merged in. The wiring was
  re-checked after the merge at 40 seeds (`circo: 40 seeds, worst 1.894e+03`, the 14 closed cases
  still exact, `oracle-graphviz --engine circo` PASS), at 2 shards for the merge path
  (`circo: 40 seeds over 2 shards, worst 1.903e+03, closed 14 exact: True`, and the merge's own
  control refuses when a shard file is hidden), and at `-Gstart` 1, 7 and 99 (three `cmp`s silent,
  exit 0, over the 40-seed fixture set). The 1000-seed rows `circo-oracle-1000` and
  `circo-merge-1000` are the orchestrator's to run, and §3's number is what they should
  reproduce. **A 40-seed worst gap is not a second measurement of the same quantity** and is not
  offered as one: the small seeds are the ones with small blocks, so 1.894e+03 says nothing about
  the 555-node seed that carries the 6.460e+04.
- **`gated` is not claimed for `layout.circular.circo`.** The row is `Status::Implemented`
  (`crates/graph-cli/src/capabilities/registry/unproven.rs`), and it stays that way whatever
  the ceiling says. §5 is the reason.
- **A 1e+05 ceiling is not agreement.** It is the next power of ten above the worst measured gap,
  which is what the job's rule prescribes, and on this engine it means the metric has stopped
  discriminating. The 14 closed cases are where the exactness actually lives, and they are
  compared byte for byte.
- **The oracle's own reader has a gap.** `capabilities/verdict.rs:63-74` matches a fixed list of
  record names and has no arm for `oracle-circo`, so `capabilities` prints
  `not backed: no oracle-circo record` even after a real run. That is a gap in the reader, the
  same one `layout.twopi` already carries, and it is **not** a claim this row is making: the
  differential is real and its numbers are above. Fixing it is `capabilities/verdict.rs`, which
  this job's paths do not include.
- **The determinism check is over 20 seeds, not 1000.** It proves the oracle's own output does not
  move with the seed, which is what §1 claims and all §3 needs. It is not a claim that the sweep
  is seed-stable, because `circo` takes no seed that reaches its answer.
- **`harness/oracle-graphviz.py` grew three child modules** (`gv_plain.py`, `gv_closed.py`,
  `gv_frames.py`) so the sharded sweep could be added without passing 300 lines, and so develop's
  own closed-case table (`osage`, whose port keeps Graphviz's translation and so needs no
  half-node offset) is one table in one file rather than a second rendering in the driver. The
  refactor was verified byte-identical: the 20-seed determinism subset through the refactored
  harness `cmp`s equal to the pre-refactor output, exit 0.

## 4c. A node placed twice (2026-10-04)

**The shape.** `longest_path` (`circo/skeleton/tree.rs:73`) builds the circle order out of two
walks up the thinned spanning forest: one from the branch node's best leaf, one from its
runner-up. When those two leaves sit in the *same* child subtree of the branch node, the walks
share the stretch between the branch node and the node they diverge below, and the node on that
stretch is named twice. The circle is then sized by the order's own length — `N` is the list's
size (`blockpath.c:568`), and it sets both the radius (`:571-576`) and every node's slot
(`theta = k * 2*PI / N`, `:600`) — so a block of 44 nodes is drawn on a 45-slot circle with one
slot empty, and the crossing walk closes that node's already-closed edges a second time and
counts them again.

**§4b's attribution is corrected here.** §4b (and the finding that prompted this job) says the
branch node's best and runner-up leaf "can be the same one". It cannot: `measure_distance`
(`blockpath.c:224-260`) executes exactly one arm per (leaf, ancestor) visit, the arms that write
`LEAFONE` (`:241`, `:250`) and `LEAFTWO` (`:253`) are mutually exclusive, and a leaf reaches any
ancestor at most once in a forest, so the two leaves are always distinct nodes. **A forest is
also not the mechanism** — a connected tree has the same shared stretch, and that is an argument
from the arithmetic below rather than a measured case, the case measured here being a forest.
The mechanism is the
arithmetic: at the branch node the sum of the two best arms is `a + b`, and at the node where
those two arms diverge it is `a + b - 2d` for the shared length `d > 0`, so the branch node wins
and its two walks overlap by construction.

**The reproducing input** is a chorded 8-cycle, as `circo/tests/path.rs` builds it: the cycle
`n0..n7` plus `n0 n3`, `n0 n4`, `n0 n6`, `n1 n4`, `n1 n5`, `n2 n4`, `n4 n7`, `n5 n7`. Its thinned
forest roots `n0` above `n1`, with two leaves under `n1` on either side, so `n0`'s two best leaves
— `n7` at distance 4 and `n3` at distance 3 — are both in `n1`'s subtree and their walks share
`n1`. `n4` is in no tree at all, so the residual pass is what names it. The port's order is
`[7, 6, 5, 4, 1, 2, 3, 0, 1]` — nine names for eight nodes, `n1` twice — and
`the_circle_order_of_a_chorded_eight_cycle_names_n1_twice_as_the_reference_does` pins it.

**What the reference does with that input: the same thing.** `circo -Tplain -Gstart=1` over the
same graph in the pinned image prints eight node lines, and fitting a circle to them gives centre
`(2.73054, 2.71860)` inches and radius `2.50669` inches, which is `180.481` points — **nine
slots** (`9 * 126 / 2*PI` = 180.4817) on eight nodes. The eight angles are `0`, `40`, `80`, `160`,
`200`, `240`, `280` and `320` degrees: slot `120` is empty and `n1` sits at slot `4`, its *first*
mention having been the empty slot `3` (a node's `POSITION` keeps its last mention, so the first
copy's slot is the one that empties). Reading the order back off those slots gives
`[7, 6, 5, 1, 1, 0, 4, 2, 3]`; this port's pre-change order is `[7, 6, 5, 4, 1, 2, 3, 0, 1]`.
Both are nine entries with `n1` twice and the same empty slot; they differ only in where
`reduce_edge_crossings` put the nodes, which is §5's disagreement and not this section's. So the
duplicate is **the reference's own behaviour, reproduced**, not a port defect — and there is no
node choice that both keeps Graphviz's answer and yields a permutation, because the overlap is
what makes the branch node the argmax.

**The decision: parity.** The job measured one alternative, `extend_once` in
`circo/skeleton/tree.rs` (the second walk adds only the nodes the order does not already name,
so the order becomes a permutation). It was **reverted** on review (2026-10-04): Graphviz engines
must match Graphviz output (user decision, 2026-09-30), and the repeat above is Graphviz's own
output. The port keeps `path.extend(second)` with a comment pointing here, and
`circo/tests/path.rs` pins the repeat: the chorded 8-cycle's order is nine entries with `n1`
twice. The rows below are the alternative's measured cost, kept so the question is not re-asked.

**The cost, measured.** The four differential rows of `scripts/orch/rows/p13-gv1-circo.rows` were
run by hand on both arms, each arm against its own fingerprint (the check row refuses a sweep whose
tree is not the tree that emitted the fixtures, so the pre-change arm was measured with the
pre-change tree in place):

```
scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine circo --seeds 1000
scripts/orch/drun ... ge-graphviz-oracle python3 harness/oracle-graphviz.py target/circo-fixtures circo target/gv-circo-shards --differential --shards 8 --shard $i   # i = 0..7
scripts/orch/drun ... ge-graphviz-oracle python3 harness/oracle-graphviz.py target/circo-fixtures circo target/gv-circo-shards --merge --shards 8
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine circo
```

| row | before | after |
|---|---|---|
| `circo-emit-1000` | exit 0, 8m43 | exit 0, 7m54 |
| `circo-oracle-1000` | 8 shards, all exit 0; shard 0 `125 seeds, worst 5.982e+04 points; closed 14 of 14 exact: True` | 8 shards, all exit 0, same line |
| `circo-merge-1000` | exit 0, `circo: 1000 seeds over 8 shards, worst 5.982e+04 points; closed 14 exact: True` | exit 0, identical line |
| `circo-check-1000` | exit 0, `PASS`, `worst 5.982e4, ceiling 1e5: ok`, `closed cases: 14 compared byte for byte: ok` | exit 0, `PASS`, identical |

Per seed, with the harness's own `gap` (`harness/gv_closed.py`) over both arms' fixtures and one
copy of the oracle's points — all eight shard files are byte-identical before and after, so the
two arms are compared against the same Graphviz:

| quantity | before | after |
|---|---|---|
| seeds within 1 point | 16 | 16 |
| worst gap | 5.9816e+04 (seed 592) | 5.9816e+04 (seed 592) |
| seeds that moved | — | 2 grew, 0 shrank, 998 unchanged |
| the two that grew | — | seed 94 and seed 694: 5.0262e+03 → 6.0838e+03 |

**What the alternative would have bought: nothing measurable.** The 16 agreeing seeds are the
same 16 on both arms, the worst case is the same seed at the same figure, and all 14 closed cases
are byte-exact on both — so no analytically determined case ever carried a repeat. Two seeds of
1000 get about one slot further from Graphviz under the alternative. With no gain and a measured
loss, parity wins.

**What this section does not claim.**

- **Blast radius is 4 seeds, not 1000.** Walking the 1000 gate models' blocks for an order that
  names a node twice finds **4**: seeds **68, 94, 668, 694**, one block each. Seed 68 is §4b's
  44-node block. The other two, 68 and 668, are in the "998 unchanged" row because a per-seed gap
  is a maximum over its nodes, so a block whose gap another node already sets cannot show the
  change. That walk was run as a scratch test over the 1000 gate models and deliberately not
  kept: 45 s in release, too slow to sit in `cargo test`, and its answer is the four seeds above.
- **Seed 68's own slot count was not read off the oracle.** A multi-block drawing's circles are
  moved and widened by `circpos` to clear each other, so fitting circles to seed 68's `-Tplain`
  coordinates gives radii that are not whole numbers of slots and says nothing. The repeat is
  *proven* on the single-block input above, where nothing moves the circle, and *derived* for seed
  68 from the same code path; it is not measured there.
- **The aggregate numbers do not say parity is free downstream.** With the repeat, the crossing
  walk closes the repeated node's edges a second time and counts them again, as Graphviz's does;
  the measured effect is the two seeds above, in the other direction.

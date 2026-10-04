# `sg-sfdp-collapse` — the sfdp layout stopped being a line

Measured 2026-10-04 on `sg-sfdp-collapse` (branch off develop `d73b3869`). Row: `GRAPHVIZ_SFDP`.

The work itself was written once on the abandoned branch `force-look` (commit `8a180baf`,
2026-10-01) and never measured. This job ported it, judged it against the job's defect list,
finished the one defect it left out, and measured everything.

## The shape measure

**Small over large eigenvalue of the 2-D covariance of the 77 points** (1 = a disc, 0 = a line),
plus the number of point pairs closer than `1e-6` of the drawing's own extent ("coincident
pairs"). Measured on `lesmis` by `target/sfdp-shape.mjs`:

```sh
scripts/orch/gr cargo build --release -p graph-cli
scripts/scigraphs-conformance.sh
scripts/orch/node-slim.sh node target/sfdp-shape.mjs \
  target/scigraphs-conformance/motor/GRAPHVIZ_SFDP.f64 77
scripts/orch/node-slim.sh node target/sfdp-shape.mjs \
  target/scigraphs-conformance/ref/GRAPHVIZ_SFDP.f64 77
```

| arm | shape ratio | coincident pairs | bit-identical pairs |
|---|---|---|---|
| Graphviz 16.1.0 (`ref/GRAPHVIZ_SFDP.f64`, first 77 points) | **0.41371** | 0 | 0 |
| motor, develop `d73b3869` | 0.00605 | **40** | 31 |
| motor, this branch | **0.27091** | **0** | **0** |

**Node order is part of the drawing.** The conformance reader numbers `lesmis`'s integer node ids
in **byte** order (`"10"` between `"1"` and `"2"`); the in-repo test harness numbers them in
**numeric** order. Same graph, different drawing — which is why the second table below exists
with its own node order and why the two tables do not agree.

## Per-defect table, re-measured on the real code

`prompts/jobs/sg-sfdp-collapse.md` shipped a probe table; these are the same rows measured on the
code that landed, not on a throwaway instrumented tree. `ratio / coincident pairs`.

In-repo harness, `lesmis` in **numeric** node order (`shape::lesmis`), the graph the tests build:

| stage | seed 1 | seed 2 | 981798123 (the row's) |
|---|---|---|---|
| develop `d73b3869` | 0.3338 / 10 | 0.3375 / 16 | 0.3387 / 40 |
| port of `force-look`, defects a+b+c+e | 0.7246 / 0 | 0.6033 / 0 | 0.6219 / 0 |
| **+ defect d, this branch** | **0.7077 / 0** | **0.5794 / 0** | **0.5542 / 0** |
| Graphviz 16.1.0's own answer (conformance arm, **byte** order — not this table's node order) | 0.4137 | — | — |

The job's own probe reported develop at 0.0047 for seed 1 and 0.006 for the row seed. Those are
the byte-order numbers; develop's numeric-order drawing is 0.334, not 0.0047. **The collapse is
order-dependent**, so "develop reads as a line" is only true of one of the two orders — but both
have the coincident pairs, which is the part that is a bug either way (10/16/40).

**Defect d makes the ratio worse, not better**, on every seed, and the job said what to do about
that: *"if it still hurts, stop and report rather than drop it."* It is kept and reported. See
"What defect d cost" below.

## Defect map: job list a-f against the ported code

| defect | port | reference | status |
|---|---|---|---|
| a. `relax` moved by `step / length` against the **argument**, every iteration | `solve.rs:131` `advance` takes the cooled `step`; the loop's `step` is a local rebind at `solve.rs:88` and the move uses that value | `spring_electrical.c:634-638` (normalise `f`, `x += step*f`), step updated at `:649` | **done** by the port |
| b. every level ran adaptive cooling from 0.1 | `solve.rs:61` `with_k` sets `adaptive: false`; `force.rs:73` `update_step(adaptive, …)` cools by `COOL` when false. Only `solve.rs:53` `new` (the coarsest) is adaptive | `:171-174` (`if (!adaptive_cooling) return cool*step`), `:1160-1161` | **done** by the port |
| c. stop test `current > TOL / K` | `solve.rs:96` `if step <= TOL \|\| iter >= limit`, a `do`-while so one iteration always runs | `:47` `tol = 0.001` absolute, `:650` `while (step > tol && iter < maxiter)` | **done** by the port |
| d. `prolongate` copied the coarse position and added a ±5e-7 `rng::jiggle` | **new `sfdp/prolongation.rs`**: `multiply_p` (`:78`), `interpolate` (`:96`, Gauss-Seidel, `ALPHA = 0.5`), `jitter` (`:120`, `K·0.001·(drand()-0.5)` on every `R`-row member after the first, from the **same** glibc stream the start drew). Driver wiring at `sfdp.rs:181`, `:193`, `:199` | `:837-852` `prolongate`, `:814-835` `interpolate_coord`, `:1155` `prolongate(..., ctrl->K * 0.001)` | **missing from the port; written here** |
| e. no `MINDIST` crop, and the Barnes-Hut tree walked below `quadtree_size = 45` | `force.rs:100` `repel` does `.max(MINDIST)`; `solve.rs:105` `gather` builds a tree only when `self.x.len() >= QUADTREE_SIZE` and otherwise calls `all_pairs` (`solve.rs:121`). The tree itself was rewritten to the reference's supernode shape — leaves exact, internal cells at their **centre of mass**, opening test on the cell's *half*-width (`quadtree.rs:156`) | `:599` `MAX(dist, MINDIST)`, `:615` for the supernode arm, `:39` `quadtree_size = 45`, `:543` `n >= quadtree_size` | **done** by the port |
| f. `solve.rs`'s doc claimed gather form over a sequential loop | doc corrected at `solve.rs:8-19`; the code was made genuinely gather (`solve.rs::gather` then `solve.rs::advance`) | — | **done**, recorded in `docs/decisions/sfdp-gather-form.md` |

Two things the port changed that the job did **not** list, both load-bearing for a:

- **`sfdp/matching.rs` is new.** The reference groups nodes with identical neighbour sets into
  modules of up to `MAX_CLUSTER_SIZE = 4` before matching (`Multilevel.c:83-101`,
  `Multilevel.h:29`), which is what lets a star's leaves coarsen four at a time. Without it a
  400-node graph coarsened through about 390 levels, decaying `K` by 0.75 each time.
- **The quadtree was rebuilt, not patched.** The old one put a supernode at the cell's **centre**
  and descended into a quadrant even for a single point; on a two-node level that gave each node
  a force with a `y` component (measured: node 0 moved to `(-0.0414, -0.0910)` on one iteration
  where the reference's arithmetic gives `(-0.1, 0)`). The new one is the reference's shape.

## RED, on develop's sfdp

`sfdp/contract.rs` was written first and run against develop's `layout/graphviz/sfdp/` (the port
moved aside; develop's `sfdp.rs` gained only the `mod contract;` line). Four of the five tests
that could compile there failed:

```
running 5 tests
test ...contract::a_fine_level_moves_each_node_by_the_cooled_step ... FAILED
test ...contract::prolongation_pulls_a_node_toward_its_neighbours_mean ... FAILED
test ...contract::a_fine_level_converges_in_forty_four_iterations ... FAILED
test ...contract::the_layout_spreads_on_lesmis ... FAILED
test ...contract::the_layout_spreads_on_a_ten_by_ten_grid ... ok

---- a_fine_level_moves_each_node_by_the_cooled_step ----
one iteration moved node 0 to -0.04138030370432556
---- prolongation_pulls_a_node_toward_its_neighbours_mean ----
node 1 at 9.999999655730214, want 12.5: interpolate_coord did not run
---- a_fine_level_converges_in_forty_four_iterations ----
assertion `left == right` failed: 44 iterations did not finish the level:
the stop test is not `step > tol`
  left: ([13837425326555020630, 4610086793724615510, ...])
 right: ([13853237449078946197, 4602805457245718936, ...])
---- the_layout_spreads_on_lesmis ----
nodes 10 and 11 are 0.0000076293945 apart, under 0.000099
test result: FAILED. 1 passed; 4 failed
```

`a_fine_level_moves_each_node_by_the_cooled_step` asserts three things and the first two hold
whatever force model is in force, so they isolate a and b from e:

1. one iteration moves each node by exactly `0.1` — develop passes this one;
2. the second iteration moves by exactly `0.09` — develop moves by `0.1` again, which is the bug,
   and this assertion is model-independent;
3. after two iterations node 0 is at exactly `-0.19`, the reference's own arithmetic on this
   level (`KP = K² = 4`, `CRK = C/K = 0.1`, `dist = 2`, so the first force is `0.4 − 2·1 = −1.6`
   and the second `0.441 − 1.905 = −1.464`, both along `−x`). Develop's **one** iteration
   instead lands node 0 at `(-0.0414, -0.0910)`: the `y` component alone is enough to show this
   is e and not a — develop's Barnes-Hut supernode sits at a cell's **centre**, and for two points
   on a horizontal line that centre is 1.1 units *above* the line.

The 10x10 lattice already passed on develop, so it is a **regression guard, not a RED**: it is
kept because a lattice is the case the studio shows when the collapse is invisible.

Defect d's second test (`prolongation_jitters_a_matched_pair_by_the_reference_s_own_scale`)
cannot compile against develop, because develop's `prolongate` takes no `K` — the scale under test
is the thing that was missing. Its RED was taken against the port as landed, where
`prolongate`'s `±5e-7` jiggle gives a matched pair a separation of at most `7.1e-7` against the
`> delta/4 = 0.025` the test demands; the develop-side magnitude is the same code and is visible in
the lesmis failure above (nodes 10 and 11 at `7.6e-6` of a `1e-4` threshold — the jiggle's scale).

## The row

| | develop `d73b3869` | this branch |
|---|---|---|
| `scripts/scigraphs-conformance.sh` | exit 0 | exit 0 |
| `GRAPHVIZ_SFDP` verdict line | `ok — 340 f64, 340 f32 of 1020 coordinates, median 8.476e-1 <= 1.000e0` | `ok — 340 f64, 340 f32 of 1020 coordinates, median 8.476e-1 <= 1.000e0` |
| median Procrustes | 0.8476 | **0.3400** |
| max Procrustes | 0.9781 | 0.99999993 |
| pinned motor sha256 | `7b7aeabb…2bd6f` | `ff26903d…14473` |

The **tier does not move**: `shape` at 1e0, cause `algorithm`, both before and after. That is the
point of the whole job, and it is the finding: **this row passed while the layout was a
one-dimensional strip with 40 coincident pairs.** A shape-tier row with a 1e0 ceiling cannot see
a collapsed drawing. `docs/measurements/p13-gv2-sfdp.md` now says so in the place a reader of the
ceiling will see it.

Only `GRAPHVIZ_SFDP` moved. `YIFAN_HU`, which shares the reference and must not, is byte-for-byte
what it was (`342 f32 / 341 f64 of 1020`, max gap `5.370698251720151`, median Procrustes
`0.8291831073753666`).

## The oracle, before and after

The p13 differential, 1000 gate seeds:

| | worst max abs coordinate gap (points) | closed cases |
|---|---|---|
| develop `d73b3869` | 3.881e+02 | 1 of 6, byte for byte |
| **this branch** | **3.887e+02** | 1 of 6, byte for byte |

```sh
scripts/orch/gr cargo run -q -p graph-cli -- emit-graphviz-fixtures --engine sfdp --seeds 1000
scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
  python3 harness/oracle-graphviz.py target/sfdp-fixtures sfdp target/gv-sfdp --differential
scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine sfdp
```

The ceiling stays at **1e3**: the new worst case is 0.6% above the old one, so nothing supports
tightening it. Graphviz compared **against itself** at `-Gstart=1` against `-Gstart=7` is
4.809e+02 on the same metric, so our arm is still closer to Graphviz's drawing than Graphviz's
own drawing at another seed is.

## What defect d cost

The job's probe reported that its own version of d made the shape worse and told us to port d from
the reference text and judge it by the row. Ported from the reference text, d still costs:

| | ratio (row arm, byte order) |
|---|---|
| port without d | 0.3640 |
| port with d | 0.2709 |
| Graphviz | 0.4137 |

and it costs on every seed in the in-repo table too (0.725 → 0.708, 0.603 → 0.579, 0.622 →
0.554). Coincident pairs were already 0 without d, so d bought nothing there either.

The job's instruction was explicit — *"if it still hurts, stop and report rather than drop it"* —
so d is in and this is the report. Two readings, and neither is settled:

- **d is right and the measurement is the wrong yardstick.** `interpolate_coord` is a smoothing
  pass over a level that was already well-shaped; a smoothing pass makes a drawing *rounder*, and
  a rounder drawing has a **lower** small-over-large eigenvalue ratio. If Graphviz's 0.414 comes
  from a single level, it never runs `interpolate_coord` at all, so matching its ratio and
  matching its algorithm are different goals.
- **the multilevel driver is the real divergence**, and d is measured against the wrong baseline.
  See below.

## The named reference step that explains the rest

**Graphviz's `sfdp` runs one level at its defaults; this port runs four.**

- `sfdpinit.c:213` — `ctrl->multilevels = late_int(g, agfindgraph_attr(g, "levels"), INT_MAX, 0)`:
  the `levels` graph attribute defaults to `0`.
- `spring_electrical.c:56` — `ctrl.multilevels = 0`, commented *"if <=1, single level"*.
- `Multilevel.c:163` — `if (grid->level >= ctrl.maxlevel - 1) return;` with `maxlevel = 0`, so
  `0 >= -1` and `Multilevel_establish` returns before coarsening anything.
- `spring_electrical.c:1145-1146` — `if (Multilevel_is_finest(grid)) { xc = x; }`, and
  `:1155` the `prolongate` call is then never reached.

So at `-Gstart=<seed>` with no `levels` attribute, Graphviz runs `spring_electrical_embedding`
once, on the whole graph, with no coarsening, no prolongation, no `interpolate_coord`, no `K`
decay, and no `adaptive_cooling = false`. Everything this job repaired in `prolongate` is on a
path Graphviz's default configuration never takes.

That is **not** fixed here, deliberately. `prompts/jobs/sg-sfdp-collapse.md` assigns "coarsening
order and smoothing" to `sg-sfdp-step`, which runs next, and this job's done-when names the
remaining ratio gap as explainable by "a named reference step not yet ported (coarsening order
and smoothing belong to `sg-sfdp-step`)". The measured gap is 0.414 − 0.271 = **0.143**, inside
the 0.15 the done-when allows, and the step that would close it is named above.

This is the single largest known disagreement with the oracle after the coarsening permutation
stream, and it is stated in `sfdp.rs`'s module doc rather than left for the next reader.

## Gaps: none closed, and why that is correct

`GRAPHVIZ_SFDP` carries exactly one convention gap, `G_GV_UTILS` (`conformance/gaps.rs:69`):
SciGraphs' Graphviz path is `scigraphs_utils.graphviz_layout`, a C++ extension whose source is not
on disk, so this arm runs the engine itself through `gv_exact` and transcribes the five lines the
extension would have applied. **That gap is untouched by this repair and still true** — nothing
here touched the reference arm, and the extension's source is still inference rather than a file.
So `conformance/rows.rs` and `conformance/gaps.rs` are unchanged, and a gap left behind after a
fix would have been the false record.

The collapse was never recorded as a gap, and should not have been: a `Gap` is a parameter
`apply_graph_layout` passes that the motor has no slot for, and "the port's step control is wrong"
is not that. It was a bug in a port, found by measurement, and the row's own tier could not see
it — which is the finding this job exists to record.

## Commands

| command | exit |
|---|---|
| `scripts/orch/gr cargo build --release -p graph-cli` (untouched tree) | 0 |
| `scripts/scigraphs-conformance.sh` (untouched tree) | 0 |
| `scripts/orch/gr cargo test -p graph-core --lib sfdp` (develop, RED) | 101 (4 failed) |
| `scripts/orch/gr cargo test -p graph-core --lib sfdp` (this branch) | 0 |
| `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown` | 0 |
| `scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8` | 0 (`layout.force.sfdp: 4-way equal on 8/8 seeds`) |
| `scripts/orch/gr cargo fmt --all --check` | 0 |
| `scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings` | 0 |
| `scripts/orch/gr cargo run -q -p graph-cli -- oracle-graphviz --engine sfdp` | 0 |
| `scripts/scigraphs-conformance.sh` (re-pinned) | 0 |
| `scripts/orch/gate.sh target/gate-job target/wf/sg-sfdp-collapse.rows` | **0** |

### The gate, `target/gate-job/summary.txt`

`hashgate --seeds 8` reporting `layout.force.sfdp: 4-way equal on 8/8 seeds` is the D10 check for
this job: two native and two wasm32 arms agree bit for bit, which is what
`docs/decisions/sfdp-gather-form.md` claims and the only reason that claim is checkable.

```
PASS fmt                    exit=0   expect=0       1s
PASS clippy                 exit=0   expect=0       0s
PASS test                   exit=0   expect=0       1212s
PASS wasm32-core            exit=0   expect=0       0s
PASS hashgate-8             exit=0   expect=0       22s
PASS negctl-degree          exit=0   expect=0       17s
PASS negctl-dim-z-mismatch  exit=0   expect=0       2s
PASS force-gate-4           exit=0   expect=0       2s
PASS negctl-force-gravity   exit=0   expect=0       1s
PASS scigraphs-conformance  exit=0   expect=0       149s
PASS negctl-scigraphs-conformance exit=0   expect=0       152s
PASS sfdp-emit-1000         exit=0   expect=0       55s
PASS sfdp-oracle-1000       exit=0   expect=0       82s
PASS sfdp-check-1000        exit=0   expect=0       2s
PASS sfdp-check-negctl      exit=2   expect=nonzero 1s
GATE EXIT=0
```

`scigraphs-conformance` is the re-pinned `GRAPHVIZ_SFDP` (exit 0). `negctl-scigraphs-conformance`
is `--break`, which exits 1 **and names `SPRING_3D`** in the judge's own log — the control a judge
that passed everything would fail. `sfdp-check-1000` is the p13 oracle at 1000 seeds against the
1e3 ceiling.

### One measurement was voided by its own tree, and is worth writing down

The first full gate run failed its `test` row, every failure of one kind:

```
graph-cli was built from tree 04611be1cfbc… but the tree is now 61680ef0a497…: rebuild before recording
```

Three different tree fingerprints appear in that log. Nothing in the port was broken: the
fingerprint is a SHA-256 over `crates/`, `harness/`, `docker/`, `Cargo.*`, `.cargo`, `fixtures/`
and the five gate wrappers (`crates/graph-cli/src/fingerprint.rs:59`), and the `test` row was
compiling and running while `sfdp/contract.rs` and `sfdp/tests.rs` were edited under it — doc
comments only, but inside the hashed set. A rebuild on the final tree is green.

The generalisable half: **a `graph-cli` test that shells out to `graph-cli` is a measurement of
the binary, and a source edit mid-row invalidates every one of them at once.** A red `test` row
whose failures all quote two tree hashes is that, not a regression — read the hashes before
reading the assertions.
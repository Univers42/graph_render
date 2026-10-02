# `SPIRAL_3D`: SciGraphs' conical 3D spiral, ported

The row that was mapped to the wrong layout. `SPIRAL_3D` was compared against
`layout.spiral` — graph-core's planar Archimedean spiral, a port of networkx's
`spiral_layout` at `resolution = 0.35` — and measured **0/1020 on both axes** with a
Procrustes disparity of 0.585. The picture said why: *grey is a 3D spiral, green one point at
the centre*.

SciGraphs has no 2D spiral. `apply_graph_layout` routes `SPIRAL_3D` to its own
`_spiral_layout_3d` (`SciGraphs/core/scigraphs_core/mesh/layouts/basic.py:36-63`) through
`layouts/dispatcher.py:105-106`, and to nothing else. So no value of `resolution` could
have moved the row, and the conformance doc's original repair 10 was wrong on both counts.
`layout.spiral` is untouched by this job.

## What was measured, and on what

The same 24 fixtures the whole matrix uses (340 nodes, 1020 coordinates per row, 23 of the
24 compared), SciGraphs' own `apply_graph_layout` at `scale = 5.0`, in the pinned
`ge-python-oracle` image (numpy 2.3.3) with the `SciGraphs/` submodule at
`b7ccee691e368425ee3154681c43fd163e572edb`. The commands and their exit codes are in
**Commands and exit codes** below.

## Before and after

| | motor id | tier | f64 k/N | f32 k/N | max ULP | max gap | Proc. median | Proc. max | cause |
|---|---|---|---|---|---|---|---|---|---|
| before | `layout.spiral` | `shape` | 0/1020 | 0/1020 | 9.22e+18 | 6 | 0.585 | 0.815 | `algorithm` |
| after | `layout.basic3d.spiral` | `tolerance` | 120/1020 | **1020/1020** | 2.68e+08 | 2.35e-07 | 3.34e-16 | 5.59e-16 | `arithmetic` |

### The run those numbers come from

The judge's own line for the row, verbatim from `target/scigraphs-conformance-judge.log`:

```text
  SPIRAL_3D: ok — 120 f64, 1020 f32 of 1020 coordinates, median 3.342e-16 <= 1.000e-15
```

and the row's cells in `target/scigraphs-conformance/metrics.json`:

```json
   "bitwise_f32": 1020,
   "bitwise_f64": 120,
   "coordinates": 1020,
   "fixtures": 24,
   "max_gap": 2.3501067669684517e-07,
   "max_ulp": 268166292,
   "measured_fixtures": 23,
   "name": "SPIRAL_3D",
   "overlays": 22,
   "procrustes_max": 5.58814278699053e-16,
   "procrustes_median": 3.3421546967776616e-16,
```

Every rounded cell in the table is one of those, rounded: `120/1020` is `bitwise_f64` over
`coordinates`, `1020/1020` is `bitwise_f32` over `coordinates`, `2.68e+08` is `max_ulp`
(268 166 292), `2.35e-07` is `max_gap`, `3.34e-16` is `procrustes_median` and `5.59e-16` is
`procrustes_max`. `fixtures: 24` with `measured_fixtures: 23` is where 24 fixtures and 23
compared come from, `overlays: 22` is the overlay count, and the `1.000e-15` in the judge line
is the row's tolerance the median is compared against.

22 of 24 fixtures overlay; the two that do not (`gate-00` at 2 nodes, `gate-01` at 3) are
below the Procrustes fit's own noise floor. `layout.spiral`'s own conformance state is
untouched — it is still networkx's curve, still `resolution = 0.35`, still its own row's
business.

**`f32 1020/1020` is the strongest statement this tree can make**, and it is the same place
`SPHERE`, `HELIX` and `HIERARCHICAL_3D` sit. `f64 120/1020` is not a shortfall in the port:
the motor arm writes its `.f64` file as the `f32` widened, so that column answers "was the
reference's own value already `f32`-representable", which is a property of numpy's output,
not of this tree. That the two counts differ is the tier rule's own argument, restated here
by measurement.

## The four numpy primitives that had to be ported, and how each was pinned

`t` is the only value in this layout that is not closed form, and reproducing it means
reproducing four numpy behaviours that each differ from the obvious translation. **numpy
2.3.3's C source is not in `$GM_SCRATCH/refs`**, so each was pinned by *reproduction* — the
candidate formula run against numpy and compared over the IEEE-754 bits — rather than by
reading `compiled_base.c`. That is a stronger pin than a citation for the thing that actually
matters, and it is the only route available offline.

| primitive | what it does | how it was pinned | result |
|---|---|---|---|
| `linspace` | `start + i*step`, last element **overwritten** with `stop` | all 65 536 grid points, bit for bit | `step = 0x3ef0001000100010` (= `1/65535`); `grid[65535] = 1.0` exactly |
| `cumsum` | **sequential**: `length[i] = length[i-1] + step` | folded into the `t` sweep below | 65 535 additions in index order |
| `interp` | `slope = (fp[j+1]-fp[j])/(xp[j+1]-xp[j])`, then `slope*(x-xp[j]) + fp[j]` | 200 000 unrelated monotone points, and every node count 1..64 plus 77, 100, 101, 600 and 601 | **0 ulp** of `np.interp` on all of them |
| `round` | half-to-**even** (`float.__round__`, `np.float64.__round__`) | `turns` pinned at n = 1..16, 77, 200, 256, 512 | half-to-even; the `max(2, ...)` floor only lifts the value at n = 1..5 |

The `t` sweep is the load-bearing one: it recomputes the whole inversion with the recovered
formulas and compares the resulting `t` column against `np.interp`'s, bit for bit, for 69
node counts (1..64, then 77, 100, 101, 600, 601). **Zero mismatches.** The pinned `t` and
`z` words in `crates/graph-core/src/layout/basic_3d/spiral/tests/reference.rs` are therefore the
reference's, not a transcription of this port's own output.

`np.interp`'s `j == lenxp - 1` case returns `fp[j]` **without a slope**, which is why
`interp`'s top-end guard is `>=` rather than `>`. Both paths give `1.0` here, so the choice
is invisible in the numbers and load-bearing in the code.

## What is not reachable, and why the tier is `tolerance`

`x` and `y` are `libm`'s `sin`/`cos` (D1) against numpy's array loops — two implementations,
two answers. Measured **within 1 `f64` ulp** at n = 1, 2 and 7, and it vanishes entirely in
the `f32` narrowing, which is why the row reaches 1020/1020 rather than stopping short. The
`f64` disagreement is not what stops this row at `tolerance`; the fact that the motor is `f32`
end to end is. `SPHERE` and `HELIX` are in exactly the same position and carry the same tier.

One number is worth stating because it looks like a bug and is not: the head node's `y` is
`-2.4492935982947065e-15`, not zero. That is `5.0 * sin(4*pi)` — `sin` of the nearest `f64`
to `4*pi` is `-4.898587196589413e-16` on **both** sides (`np.sin` and libm agree exactly
here), and the radius multiplies it. Small, non-zero, and reproducible.

## The two gaps, closed

`G_NO_ITERATIONS` and `G_SNAPSHOT_SCALE` came off the row and `G_BASIC3D_SCALE` went on,
which is what its three siblings (`SPHERE`, `HELIX`, `CUBE`) already carry. Both dropped
consts cite `at:` paths this row no longer touches — `G_NO_ITERATIONS` names
`layout/spiral.rs:63`, the planar networkx spiral, and `G_SNAPSHOT_SCALE` names the same
`basic_3d.rs:43` constant that `G_BASIC3D_SCALE` names more specifically. **Both consts stay
in `gaps.rs`**: other rows still name them, and a const left behind once nothing names it is
a false record in the other direction.

`G_BASIC3D_SCALE`'s note gained `spiral` in its list of the placements it covers. It remains
true that `scale` is a const rather than a parameter — the reference takes one `scale` with
no default of its own and no SciGraphs caller passes anything else, so a struct would be a
knob with one setting.

## The oracle arm, and what it does not yet cover

The cross-language oracle does **not** cover this layout. `harness/oracle-basic-3d.py`'s
`ARMS` and the matching table in `crates/graph-cli/src/oracle_python/basic_3d.rs:35-40`
enumerate **sphere, helix and cube only**; neither mentions `layout.basic3d.spiral`. The job
`sg-basic3d-spiral-oracle` adds the spiral arm, and it lands **after** this branch. This is a
gap in the oracle's table, not a coverage differential attached to the layout: no claim is
made or implied that the spiral is treated differently from its three siblings on purpose.

Until that job lands, the spiral row's evidence is the two things this branch did run:

- the conformance gate (`scripts/scigraphs-conformance.sh`), which compares it to SciGraphs
  byte-for-byte over 1020 coordinates (23 of 24 fixtures measured, 1020 coordinates each), and
- the graph-core unit tests, which pin the reference's own IEEE-754 words at `n = 1, 2` and 7.

## The hash-gate knob, which does not exist yet

The new stage `layout.basic3d.spiral` has **no entry in `THREE_D_LAYOUT_STAGES`**
(`crates/graph-cli/src/hashgate/knobs.rs:127`) and **no negative control in `KNOBS`**. This
job deliberately did not add one: the knob and its control belong with the stages that are
about to exist on both branches. The orchestrator's follow-up job adds the `spiral` and
`bipartite_3d` stages **and** their negative controls, once both branches are on develop.

What *was* run here is a different thing and should not be read as a knob:
`graph-cli hashgate --seeds 8`, which swept the stage as part of the default arm set and
reported

```text
layout.basic3d.spiral: 4-way equal on 8/8 seeds
```

plus the `GM_MUTATE_REFERENCE_DEGREE=9` negative control, which exited 1 with

```text
FAIL: 8 of 8 seeds diverge
```

That negative control mutates the **reference**, so it proves the seeds bite; it is not a
per-stage knob for `layout.basic3d.spiral`. Today the stage has neither a knob of its own nor
its own negative control.

The sweep was worth running precisely because a new registered layout is a new stage and this
one is not trivially target-independent: it carries a 65 536-entry `f64` table and a
`partition_point` binary search, and either could in principle depend on the target. Native run
1, native run 2, wasm32 run 1 and wasm32 run 2 all digest the same.

## Commands and exit codes

```text
scripts/orch/gr cargo build --release -p graph-cli                        -> 0
scripts/scigraphs-conformance.sh    (untouched tree)                      -> 0   PASS, 32/32 rows
scripts/scigraphs-conformance.sh    (after the port)                      -> 1   SPIRAL_3D: FAIL — motor
                                                                          bytes are not the pinned
                                                                          ones; no other row moved
scripts/scigraphs-conformance.sh    (after re-pinning)                    -> 0   PASS
      SPIRAL_3D: ok — 120 f64, 1020 f32 of 1020 coordinates, median 3.342e-16 <= 1.000e-15
scripts/scigraphs-conformance.sh --break                                 -> 1   --break caught: SPRING_3D
scripts/orch/gr cargo fmt --check                                         -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings     -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                     -> 0   all targets
scripts/orch/gr cargo run -q --release -p graph-cli -- hashgate --seeds 8 -> 0
      layout.basic3d.spiral: 4-way equal on 8/8 seeds
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 ... hashgate --seeds 8   -> 1
      FAIL: 8 of 8 seeds diverge
scripts/orch/gr cargo run -q --release -p graph-cli -- capabilities --check -> 1  see below
scripts/orch/gr cargo run -q --release -p graph-cli -- codegen --check    -> 0
scripts/orch/gr cargo test -p graph-core --lib                            -> 0   1059 passed
```

`capabilities --check` reports **70 rows, 36 problems, exit 1**. None of the 36 names
`layout.basic3d.*`; they are the whole-ledger "run the gate" class, on layouts this job never
touches, and the count is 36 both before and after this change. Two message classes, both
needing the orchestrator's own gate run:

- `gated, but no <record>: run the gate` — 17 lines.
- `gated, but hashgate ran 8 seeds, need 1000` — 19 lines. **Those 19 are this job's own
  doing**: the `hashgate --seeds 8` run above wrote `target/gates/hashgate.json` with 8 seeds,
  which reads worse than the `no hashgate record` it replaced but is the same problem. The
  orchestrator's `hashgate --seeds 1000` run overwrites it; nothing tracked by git changed.

So this row is **not** reported as passing: the command exits 1, on problems that predate this
job and that no source edit here introduced. The narrower claim being made — and the one the
done-when actually turns on — is that the new ledger row adds no problem of its own, which is
what grepping the output for `basic3d` and finding nothing establishes.

`codegen --check` reports all four generated files up to date: the four outputs are the
snapshot-header schema and its `.d.ts`, and the two `docs/contract/` schemas, and **none of
them is layout-specific**, so a new `layout.basic3d.*` id produces no codegen diff. That was
predicted before the change and confirmed by it.

## n = 0: the port and the reference disagree, deliberately

At `n = 0` the reference does **not** return an empty array. `_spiral_layout_3d(0, 5.0)`
returns shape **`(1, 3)`** — the same single point as `n = 1`. The reason is `basic.py:52`'s
guard: it is `if num_nodes > 1`, so `num_nodes = 0` falls into the **same** branch as
`num_nodes = 1` and takes `wanted = [0.5 * length[-1]]`.

`apply_graph_layout` then rejects that: `_check_positions`
(`SciGraphs/core/scigraphs_core/mesh/layouts/common.py:175-185`) raises `ValueError` because
the shape is not `(0, 3)`, and `apply_graph_layout` returns **`False`**. Verified by running
it in the pinned `ge-python-oracle` image.

This port returns an **empty 3D geometry** (three empty columns) and never fails.

That is a divergence, and it is deliberate. It is **unreachable from the conformance
matrix** — the smallest fixture there has 2 nodes, so no row in the gate can observe it. The
port chose "empty geometry" over "refuse" because `run` is
`pub(super) fn(n: u32) -> Result<Geometry, StageError>` and every other layout in `basic_3d`
is total, so a caller asking for an empty graph gets an empty drawing rather than an error,
consistently, on all four ids.

## Ponytail (what this layout is bad at)

Three, all recorded in the ledger's `ponytail` field with the failing input and the direction:

- **The turn count is floored at 2 for every `n <= 14`, but the floor only bites for
  `n = 1..5`.** Measured with numpy 2.3.3, the raw rounded `sqrt(n/(0.75*pi))` is 1 for
  `n = 1..5` and is already 2 for `n = 6..14`; `max(2, ...)` therefore lifts the value at
  `n = 1, 2, 3, 4, 5` and is a no-op from 6 to 14. The first node count whose raw round is 3
  is `n = 15`. So at the five sizes a reader tries first the count is 2 because the floor
  says so, not because the rounding did, and the "gap between successive turns close to the
  spacing along the curve" the reference's own docstring claims is still not what happens
  there. Wrong-but-plausible and silent: still a spiral, still exactly on the reference's cone.
  **An earlier version of this report claimed the floor applied across `n <= 14` and gave
  `n = 7` as a failing size. Both were wrong**: at `n = 7` the rounding already yields 2 and
  the floor changes nothing. The narrower claim is the measured one.
- **512 KiB and 65 535 sequential additions per call, for an `O(n)` output.** Reference-faithful
  and not necessary — a closed-form conic arc-length integral would be faster, would stop
  agreeing with SciGraphs' own grid, and is therefore a different layout id rather than a
  faster build of this one. Failing input: any caller running this at `n = 2` in a loop.
- **The `scale_ceiling` is inherited, not measured.** `BASIC_3D_CEILING = 1_000_000` was
  benchmarked against `sphere`, `helix`, `cube` and `hierarchical3d`
  (`registry/three_d.rs:28-42`); this layout was never run at 1 M nodes or any size by
  `graph-cli bench`. The extrapolation is sound on the output side — `O(n)` in three columns,
  no graph, no iteration, like `sphere` — and not on the input side, which is where this
  layout differs: the 65 536-entry table is paid on **every** call whatever `n` is, and no
  measured figure at that ceiling has ever paid it. Nothing at or past 1 M nodes has been run
  for this layout, so the number understates the wall by an unmeasured amount. Recorded as a
  `Ponytail:` line on the ledger row.

No seed is owed and none is published: the function draws no random number at all.

## Left behind

Two things are deliberately not done here, both of them follow-up work rather than omissions
with a reason to be silent about them:

- **`harness/oracle-basic-3d.py` has no `--function spiral`**, and
  `crates/graph-cli/src/oracle_python/basic_3d.rs`'s own doc still describes the arm as
  covering three placements. Job `sg-basic3d-spiral-oracle` adds the arm. Until it lands the
  `--function` selector has no spiral to select, so the one-line doc change that would advertise
  it would be a lie until the code exists.
- **The hash-gate stage has no knob or negative control** — see the section above.

The stale "three" prose that *could* be corrected without claiming unimplemented behaviour has
been corrected in this round: `crates/graph-cli/src/oracle_python/cli.rs` and
`crates/graph-core/src/layout/basic_3d.rs`'s module doc now say which three the arm covers and
that the spiral is not among them.

## Files

- `crates/graph-core/src/layout/basic_3d/spiral.rs` + `spiral/tests{,/reference,/structure}.rs`
  — the port and its tests; `basic_3d.rs` — the `spiral` wrapper and the module doc
- `crates/graph-core/src/registry/three_d/{,basic,spiral3d,graph}.rs` — the `SPIRAL_3D`
  `Metadata`, and the split that keeps every file under 300 lines; `registry.rs` — the
  appended `Capability`, `LAYOUTS: 35 -> 36`
- `crates/graph-cli/src/oracle_python/conformance/{rows,gaps}.rs`, `baseline/table/networkx.rs`
- `crates/graph-cli/src/capabilities/{registry/unproven,tests/registry{,/ids}}.rs` — the
  routing, and the two id lists that keep it honest
- `crates/graph-cli/src/{snapshot_cmd/tests,hashgate/tests/report}.rs` — the id lists and
  record fixtures, which enumerate every registered layout
- `crates/graph-cli/src/oracle_python/cli.rs`, `oracle_python/basic_3d.rs` docs — which three
  placements the coverage arm actually arms
- `docs/measurements/scigraphs-conformance.md` — matrix row 14 and repair 10
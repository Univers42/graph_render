# sg-graphviz-scale — SciGraphs' five lines over the engine's points, and then the rounding

**Job:** `prompts/jobs/sg-graphviz-scale.md`. **Decision it implements:** the Graphviz ports in
`graph-core` match Graphviz 16.1.0 and are gated against it, so this job changed **nothing**
under `crates/graph-core`; SciGraphs' layer below the engine's output belongs to the conformance
arm. (User decision, 2026-09-30.)

**Review round 1 changed two claims in the first version of this report, and both were wrong.**
"`-Tplain` writes inches at five decimals" was wrong: `printdouble` is
`agxbprint(&buf, "%.5g", v)` (`lib/common/output.c:66-71`), five **significant** digits, so the
step is `10^(floor(log10|v|) - 4)` inches — `7.2e-3` points for a coordinate in [1, 10) in, ten
times the `7.2e-4` grid this report used to claim. And "numpy's own pairwise summation" was
wrong: `pairwise_sum_DOUBLE` only runs along the **contiguous** axis, and `raw.mean(axis=0)` on
a C-contiguous `(n, 3)` reduces the strided one. Section 3 pins the corrected order against
numpy and section 4 is the re-measure that took both rows past `bitwise`.

## 1. Measure: the units were never the difference

On the untouched tree, for one fixture set, the ratio between the motor arm's span and the
`-Tplain` arm's span (`target/scigraphs-conformance/{motor,ref}/GRAPHVIZ_*.f64` sliced by
`conformance.jsonl`):

| row | fixture | motor span | `-Tplain` span | ratio |
|---|---|--:|--:|--:|
| `GRAPHVIZ_TWOPI` | `lesmis` (77) | 519.724, 521.310 | 519.727, 521.309 | 0.999994, 1.000002 |
| `GRAPHVIZ_TWOPI` | `tree-balanced` (15) | 399.116, 399.116 | 399.118, 399.118 | 0.999996, 0.999996 |
| `GRAPHVIZ_PATCHWORK` | `lesmis` (77) | 247.661, 247.394 | 247.665, 247.396 | 0.999985, 0.999994 |
| `GRAPHVIZ_PATCHWORK` | `tree-balanced` (15) | 93.804, 96.789 | 93.807, 96.787 | 0.999975, 1.000022 |

**Both sides are in points, and the line that says so is the conversion itself:**

- `harness/gv_plain.py:24` — `POINTS_PER_INCH = 72.0`
- `harness/gv_plain.py:92` — the graph line's `width`/`height`, `× POINTS_PER_INCH`
- `harness/gv_plain.py:95-96` — each node's `x` and `y`, `× POINTS_PER_INCH`
- `harness/gv_plain.py:82-84` — *"the graph line's size in points and every node's centre in points"*
- `harness/oracle-graphviz.py:13` — *"-Tplain's output format reports inches; 1 inch = 72 points"*

So the job body's hypothesis — "the matrix compares two unit systems for all eight rows" — is
**false for the units**. The ratio is 1.0000 to five decimals.

**What is real is the origin.** The ports centre their own layout; the engine reports in the
page frame:

| row | fixture | motor mean | `-Tplain` mean | delta |
|---|---|--:|--:|--:|
| `GRAPHVIZ_TWOPI` | `lesmis` | 12.617, 3.797 | 283.125, 263.335 | −270.508, −259.537 |
| `GRAPHVIZ_TWOPI` | `tree-balanced` | 0.000, 0.000 | 226.558, 217.558 | −226.558, −217.558 |
| `GRAPHVIZ_PATCHWORK` | `lesmis` | 0.000, 0.000 | 138.744, 138.744 | −138.744, −138.744 |

`twopi`'s `max gap` of 303 is that translation, not a scale. The one row where a scale *is*
different is `GRAPHVIZ_NEATO` (motor span 5.566 against the engine's 417.240 on `lesmis`); it is
`rng`, it belongs to `sg-neato-start`, and the transform normalises it as a side effect.

**The floor, and what it really is.** `-Tplain` prints every coordinate through `printdouble`,
which is `agxbprint(&buf, "%.5g", v)` (`lib/common/output.c:66-71`, `printpoint` at `:76-79`
passes it inches). `%.5g` is **five significant digits, not five decimals** — `twopi -Tplain` on
a triangle prints `node a 0.375 1.25` and `edge a b … 1.5023`, and `1.5023`/`0.50234` are five
digits each — so the step is `10^(floor(log10|v|) - 4)` inches and depends on the coordinate's
magnitude:

| coordinate | step in inches | step in points |
|---|--:|--:|
| in [0.1, 1) | 1e-5 | 7.2e-4 |
| in [1, 10) | 1e-4 | 7.2e-3 |
| in [10, 100) | 1e-3 | 7.2e-2 |

**That grid belongs to this arm, not to SciGraphs**, and the reason is an **inference, not a
measurement**: `graphviz_layout(num_nodes, edges, engine=..., ...)` (`yifan_hu.py:298-307`) is
handed a node count and an edge list and returns an array, so on this reading it is a layout call
rather than a rendering, and a rendering is what rounds. Nothing on disk settles it —
`scigraphs_utils`'s source is not in the repository, only the `scigraphs-utils==0.2.0` pin at
`SciGraphs/constraints/linux-x64.txt:21`. One step of the [1, 10) in step, after the centring
and the rescale, is `7.2e-3 / 519.7 × 5 = 6.9e-5` on `lesmis` and `7.2e-3 / 216 × 5 = 1.7e-4`
on `gate-03` (half-step `8.3e-5`) — **ten times** the `6.9e-6` and `1.7e-5` the first version of
this report claimed.

## 2. RED, then GREEN: one function, two arms

`scigraphs_graphviz_post(points, dims, scale)` in
`crates/graph-cli/src/oracle_python/conformance/motor/gv_post.rs`, plus the same five lines in
`harness/scigraphs-conformance/sc_graphviz.py` (`_scigraphs_columns`), applied to every
`Reference::Graphviz` row by `motor::run_row`.

Six tests, all in `motor/gv_post/tests.rs`, with every constant pasted from numpy in
`ge-python-oracle` as `struct.pack("<d", v).hex()` — the IEEE-754 double, little-endian.
`float.hex()` was the first choice and is wrong to paste: it is `PyOS_double_to_string`'s mode 3,
which keeps trailing zeros and can write a fourteenth fraction digit, so one double has several
spellings and a typo in the padding cannot be seen.

- `three_points_land_where_numpy_puts_them` — three points at `dims = 2, scale = 5.0`, all nine
  coordinates. Observed RED on the untouched tree: `lowercase hex` was not even a function of
  `f64` (`f64: LowerHex` is not implemented), which is what sent the constants to bit patterns.
- `a_third_column_is_dropped_rather_than_moved` — the same three with a `z` each; numpy's
  answer is identical, because `positions = np.zeros((num_nodes, 3))` (`:323`) drops it.
- `a_layout_with_no_extent_is_all_zeros_and_not_nan` — `extent if extent > 0 else 1.0` (`:321`).
- `seventeen_nodes_land_where_numpy_puts_them` — all 51 coordinates of a 17-node `(17, 2)`
  array through all of `:318-325`, long enough for the mean's order to show in the output.
- `the_mean_of_an_n_by_2_array_is_the_left_to_right_sum` — section 3.
- `a_graphviz_row_is_written_centred_and_at_the_scale` — end to end on `GRAPHVIZ_TWOPI` over
  `bipartite`, with the raw port output as its own control.

## 3. The summation order: left to right, and the 1-D probe could not see it

**`raw.mean(axis=0)` over a C-contiguous `(n, 3)` is a plain left-to-right sum of each column,
not a pairwise sum.** numpy's `pairwise_sum_DOUBLE` — eight accumulators up to
`PW_BLOCKSIZE = 128`, combined as `((r0+r1)+(r2+r3)) + ((r4+r5)+(r6+r7))`, and a split in two
at an eight-aligned midpoint above that — is only reached when the reduction runs along the
**contiguous** axis. SciGraphs reduces axis 0 (`yifan_hu.py:318`), which is the strided axis of
a C-contiguous array, so the pairwise path never runs and the eight-accumulator block is not
what this arm reduces with.

The first version of this report asserted the opposite, and its evidence could not have caught
it: the probes were **1-D**, where axis 0 *is* the contiguous axis and pairwise does apply, and
the only 2-D check was `n = 3`, below the `n < 8` branch where both orders are the same code.

**Pinned, on the array shape SciGraphs actually reduces.** Measured in `ge-python-oracle` on
`(n, 2)` arrays built C-contiguous (`arr.flags['C_CONTIGUOUS']` asserted per row), at
n = 1, 2, 3, 5, 8, 9, 16, 17, 33, 64, 127, 128, 129, 300, on **both** columns:

| | `raw.mean(axis=0)` equals left-to-right | equals a pairwise sum |
|---|--:|--:|
| n = 1, 2, 3, 5 | yes | yes (identical — below eight the pairwise branch *is* left-to-right) |
| n = 8 … 300, column 0 | **yes** | **no, at every length** |
| n = 8, 9, 64, column 1 | yes | no |
| n = 16, 17, 33, 127, 128, 129, 300, column 1 | yes | no |

`the_mean_of_an_n_by_2_array_is_the_left_to_right_sum` pastes all 28 of numpy's own hex values,
asserts left-to-right against them, and uses the transcribed pairwise sum as its **negative
control**: it must differ at every `n >= 8` on column 0, and must agree below eight. Observed
by mutation — putting the pairwise order back into `left_to_right` fails the run with

```
the_mean_of_an_n_by_2_array_is_the_left_to_right_sum ... FAILED
  panicked at tests.rs:98: assertion `left == right` failed: n = 8, column 0: not numpy's mean
seventeen_nodes_land_where_numpy_puts_them ... FAILED
test result: FAILED. 4 passed; 2 failed
```

(exit 101). `gv_post.rs` no longer contains the pairwise transcription at all; it is a
negative control and lives in `tests.rs`.

`harness/scigraphs-conformance/sc_graphviz.py`'s `_numpy_mean` is the same reduction in plain
Python, and its module docstring carries the same `Ponytail:` note naming the order and the
evidence. **What the note says it gets wrong:** a Fortran-ordered `raw` would restore the
pairwise sum, and `scigraphs_utils` is a C++ extension with no source on disk to read its
allocation from — so "C-contiguous" is the assumption, and the escape hatch is to branch on the
array's flags rather than its shape, which nothing here can do.

**How much the order is worth, measured both ways.** A mean error of `n·eps·|x| ≈ 4e-12` points
on `lesmis` becomes `4e-14` after the rescale, against a `7.2e-3`-point reference quantum —
nine orders of magnitude under it. So the order is not what moved these rows; it is pinned
because a convention that depends on a summation order is not a convention.

## 4. Past `partial`: the reference reads `ND_coord`, not the `-Tplain` text

The first version stopped at "the tier did not move, and the reason is measured: the reference
arm reads the engine's **text**". That reason was right about the floor and wrong about the
cure — it said closing it needs `scigraphs_utils` in an image. It does not.

**Every Graphviz text output rounds**, and each with its own format: `plain`/`plain-ext` are
`%.5g` in inches (`output.c:66-71`), `dot`/`json` positions are `%.5g` in points
(`output.c:294,302`), `xdot` is `%.02f`, the json `draw` operations are `%.03f`, and pygraphviz
goes through `-Tdot`. So the fix is not to read a different format; it is to read no format.

**`harness/scigraphs-conformance/gv_exact.c`** is a 40-line C reader that links libgvc, reads
the DOT with `agread`, sets the seed as the graph attribute `start` the way `-Gstart=N` does
(`lib/common/input.c:281-286` → `global_def`, `:178-192`; `setSeed` reads it back at
`lib/neatogen/neatoinit.c:921`), calls `gvLayout(gvc, g, engine)` (`lib/gvc/gvc.c:52-64`, which
is `gvlayout_select` followed by `gvLayoutJobs`), and prints

```
printf("%s %a %a\n", agnameof(n), ND_coord(n).x, ND_coord(n).y);
```

`%a` is hex float, an exact round trip, so not one bit is lost between `gvLayout` and the matrix.
`gv_exact.py` compiles it once per process with the image's own gcc
(`-I/opt/graphviz/include -L/opt/graphviz/lib -lgvc -lcgraph -lcdt -Wl,-rpath`) and parses it
with `float.fromhex`. This is why `docker/graphviz-oracle.Dockerfile` **keeps** `build-essential`
in a single-stage image, and why its header comment no longer claims the image carries no
compiler.

**It is the same layout, and that is measured rather than asserted.** `gv_exact.py`'s own
doctest compares both readings on a triangle and a 77-node ring over all eight engines; the
residue, which is exactly the `%.5g`:

| engine | ring `dx` pt | ring `dy` pt |
|---|--:|--:|
| `twopi` | 1.94e-04 | 0.00e+00 |
| `patchwork` | 3.59e-03 | 3.55e-03 |
| `dot` | 1.94e-04 | 0.00e+00 |
| `sfdp` | 3.54e-03 | 3.50e-03 |
| `osage` | 3.57e-03 | 3.20e-03 |
| `neato` | 3.40e-02 | 3.60e-02 |
| `fdp` | 3.60e-02 | 3.55e-02 |
| `circo` | 3.57e-02 | 3.59e-02 |

and the converse check closes it: applying `"%.5g" % (exact/72)` to the exact coordinates
reproduces `-Tplain`'s printed value with `0.000e+00` in error for all eight engines, against
a raw difference of up to `4.99e-04` in. A seed negative control (`exact@7` vs `plain@1`)
diverges by `1.06e+02`–`2.53e+02` pt, so the seed is plumbed and not ignored.

**Re-measured, both rows.**

```
scripts/scigraphs-conformance.sh  ->  exit 0     (32/32)
scripts/scigraphs-conformance.sh --break  ->  exit 1   (SPRING_3D FAIL, the nine Graphviz rows ok)
```

| row | `max gap` on `-Tplain` | `max gap` on `ND_coord` | `f32` before | `f32` after | tier before | tier after |
|---|--:|--:|--:|--:|---|---|
| `GRAPHVIZ_TWOPI` | 7.46e-05 | **1.70e-07** | 362/1020 | **841/1020** | `bitwise`/`convention` | **`tolerance`/`arithmetic`** |
| `GRAPHVIZ_PATCHWORK` | 2.37e-04 | **1.81e-07** | 388/1020 | **831/1020** | `bitwise`/`convention` | **`tolerance`/`arithmetic`** |

Per fixture, on the two rows that had a floor:

| row | fixture | `max gap` before | after | Procrustes median before | after |
|---|---|--:|--:|--:|--:|
| `GRAPHVIZ_TWOPI` | `lesmis` (77) | 7.46e-05 | 1.16e-07 | 2.04e-10 | 6.03e-16 |
| `GRAPHVIZ_TWOPI` | `tree-balanced` (15) | — | 1.98e-08 | — | 3.87e-17 |
| `GRAPHVIZ_TWOPI` | `gate-03` (5) | — | 2.38e-08 | — | 2.41e-17 |
| `GRAPHVIZ_PATCHWORK` | `lesmis` (77) | 2.37e-04 | 1.24e-07 | 4.32e-10 | 4.69e-16 |
| `GRAPHVIZ_PATCHWORK` | `tree-balanced` (15) | — | 8.49e-08 | — | 4.87e-16 |
| `GRAPHVIZ_PATCHWORK` | `gate-03` (5) | — | 6.07e-08 | — | 1.64e-16 |

**Why `tolerance` and not `bitwise`, with the reason.** The job asked for bitwise or a measured
tolerance. This is the measured tolerance, and what is left in it is named rather than
hand-waved. `sc_propose.ARITHMETIC_GAP` is `1e-6` and both rows' `max gap` is now `1.7e-7`, an
order of magnitude inside it; the residual is the twopi/patchwork **port's own** arithmetic
against the engine's, not the reference's formatting — `gate-00` on both rows is `5.9e-17` and
`0.0` respectively, which is `f64` rounding, and the worst fixture is the one with the most
nodes. `1020/1020` bitwise `f64` is not reachable from here: `GRAPHVIZ_TWOPI` reaches 364/1020
on `f64` and 841/1020 on `f32`, and the last 179 `f32` coordinates differ because the motor's
narrowing and the reference's differ about which `f32` a value rounds to. That is what
`tolerance` means, and the tier is the honest one.

**The other seven rows did not move**, and the reason is the same as before: their `max gap` is
a different picture, hundreds of points apart, so no reference precision reaches it. Their
`tier` and `cause` are untouched; only their `reference_sha256` moved, because the transform is
on the reference arm.

| row | `max gap` | tier | cause |
|---|--:|---|---|
| `YIFAN_HU` | 5.37 | `shape` | `algorithm` |
| `GRAPHVIZ_NEATO` | 5.59 | `bitwise` | `rng` |
| `GRAPHVIZ_FDP` | 5.72 | `bitwise` | `rng` |
| `GRAPHVIZ_SFDP` | 6.87 | `shape` | `algorithm` |
| `GRAPHVIZ_CIRCO` | 5.08 | `shape` | `algorithm` |
| `GRAPHVIZ_OSAGE` | 5.02 | `shape` | `algorithm` |

## 5. Result, and what is still not SciGraphs' answer

- **Two tiers moved, `bitwise`/`convention` → `tolerance`/`arithmetic`,** and `GRAPHVIZ_DOT`'s
  stayed `shape`/`reference-absent` with its motor still the empty file. Nine rows re-pinned:
  seven `reference_sha256`, two `motor_sha256` as well (the mean's order changed the motor's
  own bytes), and two tiers.
- **`G_GV_UTILS` narrowed again.** Its `parameter` is still `reference arm`, and what it says
  is now that the reference runs the engine itself through `gv_exact.c` rather than reading
  `-Tplain`, and that what remains missing is `scigraphs_utils` — what it does to the
  coordinates between `gvLayout` and the array it returns, and what seed it passes down. Both
  are marked INFERENCE with the `constraints` pin cited, because the extension's source is not
  on disk.
- **`G_GV_Z` is untouched** and still the one const carrying `sfdp_dim`'s `"2Z"` default for
  `YIFAN_HU` alone.
- **The oracle image has no numpy** (`docker/graphviz-oracle.Dockerfile`), so the reference
  arm's five lines are transcribed in plain Python rather than executed. The two transcriptions
  are held together by the Rust test against numpy's own output, not by agreement.

## 6. Commands

```
scripts/orch/gr cargo build --release -p graph-cli                     -> 0
scripts/orch/gr cargo test -p graph-cli --release gv_post              -> 0  (6 passed)
scripts/orch/gr cargo test -p graph-cli --release gv_post  [negctl]    -> 101  (2 failed: pairwise order)
scripts/scigraphs-conformance.sh                                       -> 0  (32/32)
scripts/scigraphs-conformance.sh --break                               -> 1  (SPRING_3D FAIL)
scripts/orch/gr cargo fmt --all --check                                -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings  -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                  -> 0
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8        -> 0
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8 -> 1
docker run --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \
  python3 harness/scigraphs-conformance/gv_exact.py                     -> 0
```
# sg-graphviz-scale — SciGraphs' five lines over the engine's points, on both arms

**Job:** `prompts/jobs/sg-graphviz-scale.md`. **Decision it implements:** the Graphviz ports in
`graph-core` match Graphviz 16.1.0 and are gated against it, so this job changed **nothing**
under `crates/graph-core`; SciGraphs' layer below the engine's output belongs to the conformance
arm. (User decision, 2026-09-30.)

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

**The floor, measured separately.** `-Tplain` writes **inches at five decimals** —
`twopi -Tplain` on a triangle gives `node a 0.375 1.25` and `edge a b … 1.5023` — so every
reference coordinate is a multiple of `1e-5 × 72 = 7.2e-4` points. **That grid belongs to this
arm, not to SciGraphs:** `graphviz_layout(num_nodes, edges, engine=..., ...)`
(`yifan_hu.py:298-307`) is handed a node count and an edge list and returns an array, so it is a
layout call rather than a rendering, and a rendering is what rounds. One step of the grid, after
the centring and the rescale, is `7.2e-4 / 519.7 × 5 = 6.9e-6` on `lesmis` and
`7.2e-4 / 216 × 5 = 1.7e-5` on `gate-03`.

## 2. RED, then GREEN: one function, two arms

`scigraphs_graphviz_post(points, dims, scale)` in
`crates/graph-cli/src/oracle_python/conformance/motor/gv_post.rs`, plus the same five lines in
`harness/scigraphs-conformance/sc_graphviz.py` (`_scigraphs_columns`), applied to every
`Reference::Graphviz` row by `motor::run_row`.

Five tests, all in `motor/gv_post/tests.rs`, with every constant pasted from **numpy 2.3.3 in
`ge-python-oracle`** as `struct.pack("<d", v).hex()` — the IEEE-754 double, little-endian.
`float.hex()` was the first choice and is wrong to paste: it is `PyOS_double_to_string`'s mode 3,
which keeps trailing zeros and can write a fourteenth fraction digit, so one double has several
spellings and a typo in the padding cannot be seen.

- `three_points_land_where_numpy_puts_them` — three points at `dims = 2, scale = 5.0`, all nine
  coordinates. Observed RED on the untouched tree: `lowercase hex` was not even a function of
  `f64` (`f64: LowerHex` is not implemented), which is what sent the constants to bit patterns.
- `a_third_column_is_dropped_rather_than_moved` — the same three with a `z` each; numpy's
  answer is identical, because `positions = np.zeros((num_nodes, 3))` (`:323`) drops it.
- `a_layout_with_no_extent_is_all_zeros_and_not_nan` — `extent if extent > 0 else 1.0` (`:321`).
- `the_sum_here_is_numpys_own_order` — six lengths (9, 16, 17, 33, 129, 300) bracketing every
  branch of numpy's reduction, with **the negative control being `left_to_right`, the same
  function the `n < 8` branch runs**: it has to differ from numpy at every length or the order
  is not load-bearing and the test says nothing.
- `a_graphviz_row_is_written_centred_and_at_the_scale` — end to end on `GRAPHVIZ_TWOPI` over
  `bipartite`, with the raw port output as its own control.

### The summation order, and which one this is

**numpy's own order, not a left-to-right sum.** `mean` reduces through `pairwise_sum_DOUBLE`: a
plain sum below 8 elements, eight accumulators up to `PW_BLOCKSIZE = 128` combined as
`((r0+r1)+(r2+r3)) + ((r4+r5)+(r6+r7))`, and above 128 a split in two at an eight-aligned
midpoint. Measured on cancellation-heavy probes, the left-to-right sum of the same `n = 300`
values gives a mean of `0x1.df04444444444p-7` against numpy's `0x1.999999999999ap-7`.

The Rust test holds this to numpy at all six lengths, and the Python transcription was checked
against numpy in `ge-python-oracle` at the same six lengths — all six equal, and the three-node
transform equal as well. Both arms of a row therefore reduce the mean the same way.

**How much it is worth here: less than the `-Tplain` grid.** A mean error of `n·eps·|x| ≈ 4e-12`
points on `lesmis` becomes `4e-14` after the rescale, against a `7.2e-4` reference quantum. It
is done because a convention that depends on a summation order is not a convention, and the test
that proves it is the same one that would have caught getting it wrong.

## 3. Result

```
scripts/scigraphs-conformance.sh  ->  exit 0     (32/32)
scripts/scigraphs-conformance.sh --break  ->  exit 1   (SPRING_3D FAIL, the nine Graphviz rows ok)
```

| row | `max gap` before | `max gap` after | `f32` before | `f32` after | tier | cause |
|---|--:|--:|--:|--:|---|---|
| `GRAPHVIZ_TWOPI` | 302.878 | **7.46e-05** | 340/1020 | 362/1020 | `bitwise` | `convention` |
| `GRAPHVIZ_PATCHWORK` | 138.748 | **2.37e-04** | 340/1020 | 388/1020 | `bitwise` | `convention` |
| `YIFAN_HU` | 508.180 | 5.37 | 340/1020 | 342/1020 | `shape` | `algorithm` |
| `GRAPHVIZ_NEATO` | 472.722 | 5.59 | 340/1020 | 340/1020 | `bitwise` | `rng` |
| `GRAPHVIZ_FDP` | 2063.564 | 5.72 | 344/1020 | 340/1020 | `bitwise` | `rng` |
| `GRAPHVIZ_SFDP` | 345.230 | 6.87 | 345/1020 | 340/1020 | `shape` | `algorithm` |
| `GRAPHVIZ_CIRCO` | 6426.900 | 5.08 | 340/1020 | 351/1020 | `shape` | `algorithm` |
| `GRAPHVIZ_OSAGE` | 497.974 | 5.02 | 387/1020 | 355/1020 | `shape` | `algorithm` |

The six rows whose *cause* is not `convention` now have a `max gap` bounded by the unit box
(~5.7), which is what "a different picture" looks like once the origin is gone. That is the
whole difference: they were already different pictures, 500 points apart.

**Two `f32` counts fell rather than rose, and that is not a regression.** `GRAPHVIZ_FDP` 344 ->
340 and `GRAPHVIZ_SFDP` 345 -> 340: before the transform those coordinates sat at hundreds of
points, where an `f32` ULP is `6e-5`, so two coordinates that were far apart in absolute terms
were nevertheless equal once narrowed. Inside the unit box an `f32` ULP is `2.4e-7` and the same
accidental narrowing does not happen. The `max gap` column is the honest one for those rows, and
it fell by two orders of magnitude.

**The tier did not move, and that is the honest result.** The job's done-when asked for
`f32 1020/1020` or `tolerance` on the two `convention` rows. Neither is reachable here, and the
reason is measured, not argued: the reference arm reads the engine's **text**, whose five
decimals put every coordinate on a `7.2e-4`-point grid, one step of that grid is `1.7e-5` after
the rescale on a five-node fixture, and the motor's own `f32` narrowing adds `1.5e-7` on top.
`sc_propose.classify` reaches `tolerance` only at `max gap <= 1e-6` (`ARITHMETIC_GAP`), so both
rows stay `bitwise`/`convention` — correctly, because bytes *are* reachable and the only thing
in the way is the substitute reference. Closing that needs `scigraphs_utils` in an image, not a
layout, and no job on the list has that.

## 4. The other six rows' disparity did not move

The transform is a per-arm similarity, and `scipy.spatial.procrustes` centres and unit-normalises
both clouds, so the disparity is invariant by construction. Measured, worst per fixture:

| row | before | after | delta |
|---|--:|--:|--:|
| `YIFAN_HU` | 0.9851733805845224 | 0.9851733805845226 | +2.2e-16 |
| `GRAPHVIZ_NEATO` | 0.9501827673967730 | 0.9501827673967729 | −1.1e-16 |
| `GRAPHVIZ_FDP` | 0.9438514922897007 | 0.9438514922897003 | −3.3e-16 |
| `GRAPHVIZ_SFDP` | 0.9780886212841106 | 0.9780886212841107 | +1.1e-16 |
| `GRAPHVIZ_CIRCO` | 0.8748931494231260 | 0.8748931494231259 | −1.1e-16 |
| `GRAPHVIZ_OSAGE` | 0.9635344983986608 | 0.9635344983986608 | 0 |

One or two ULPs of an `f64`, i.e. **2.2e-16 to 3.3e-16 absolute**, thirteen orders of magnitude
under the `1e-12` the job asked for. The two `convention` rows move by `1e-21` to `1e-22`
absolute on disparities of `1e-10`, from the centring and the division re-rounding the last bit
of each coordinate before the fit.

## 5. What else moved, and what was left behind

- **Nine rows re-pinned, not eight.** `GRAPHVIZ_DOT`'s motor is still the empty file and its tier
  and cause are untouched, but its *reference* bytes moved too — the transform is on the
  reference arm — so its `reference_sha256` had to move with the other eight. The judge named
  exactly these nine and nothing else.
- **`G_GV_UTILS` narrowed, not closed.** Its `scale` claim is gone (both arms now multiply by
  `SCALE`). What is still true is narrower and is now what it says: the arm is the **engine
  binary** rather than `scigraphs_utils.graphviz_layout`, and `-Tplain`'s five decimals are the
  floor under the two `convention` rows. Its `parameter` changed from `scale` to `reference arm`,
  which is what it has been since `G_GV_DIRECTED` set the precedent.
- **`G_GV_Z` added, for `YIFAN_HU` only.** `sfdp_dim` defaults to `"2Z"`
  (`yifan_hu.py:357`), so SciGraphs replaces the z with a spectral component
  (`yifan_hu.py:327-334`) and both arms write `0.0`. That fact used to live inside
  `G_GV_UTILS`'s `scale` note; narrowing that note would have deleted it, so it has its own
  const. The other eight rows pass `dimension=None` and take the `"2"` default.
- **The oracle image has no numpy.** `ge-graphviz-oracle` is `debian:trixie-slim` plus a Graphviz
  build and nothing else (`docker/graphviz-oracle.Dockerfile`), so the reference arm's five lines
  are transcribed in plain Python rather than executed. The two transcriptions are held together
  by the Rust test against numpy's own output, not by agreement.

## 6. Commands

```
scripts/orch/gr cargo build --release -p graph-cli                     -> 0
scripts/orch/gr cargo test -p graph-cli --release gv_post              -> 0  (5 passed)
scripts/scigraphs-conformance.sh                                       -> 0  (32/32)
scripts/scigraphs-conformance.sh --break                               -> 1  (SPRING_3D FAIL)
scripts/orch/gr cargo fmt --check                                      -> 0
scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings  -> 0
scripts/orch/gr cargo test --workspace --no-fail-fast                  -> 0
```

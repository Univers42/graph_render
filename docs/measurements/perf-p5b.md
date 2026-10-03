# Perf P5b — the settled picture on the GPU, and what the fill really costs

Two rounds. Round 1 moved the settled picture onto the GPU and reverted it: it was 2x slower here.
Round 2 pulled the lever round 1's profile named — the number of readbacks, not the copies — and
kept it: **−26% on the median time to a full settled picture at 1M nodes**, with the pan's own
frames the same length either side. The pixels of the settled picture are *not* bit-identical
across the change, and the section on that says why.

Measured 2026-10-02 in worktree `perf-p5b`, host dlesieur42 (20 cores, load average 13–38 from
other jobs), image `gm-chromium` (Chrome 154.0.8037.57, viewport 1920x1080, DPR 1). WebGL2 runs
on SwiftShader, Chrome's CPU rasteriser: there is no GPU in the container. Base commit `9725eb5`.

```sh
scripts/studio.sh build
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/settle.py 1000000 webgl2 <label>
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/settle-profile.py 1000000 webgl2 <label>
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/settle-pan.py 1000000 webgl2 <label>
PERF_MEMORY=10g scripts/studio-perf.sh --label p5b-reverted --cases 200000,1000000 \
  --layout layout.random --backend webgl2
scripts/studio.sh check
scripts/studio-backend.sh                          # the backend gate
STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh   # its negative control: expect non-zero
scripts/studio-smoke.sh
STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh       # its negative control: expect non-zero
```

**Caveat:** SwiftShader is a CPU rasteriser sharing the host with other jobs, so these numbers
rank builds on this host and say nothing about a real GPU, where the readback that dominates
them is nearly free. The host's spread is large enough to name: the same build, unchanged,
read 51.1 and 38.4 worst fps at 200 000 nodes in two `studio-perf` runs hours apart. Only the
interleaved A/B below reads; a single run of either build does not.

## The result: the GPU picture is slower here, and it is reverted

The settled picture was moved from a kept 2D `OffscreenCanvas` onto two GPU textures the size of
the canvas — one for the edges, one for the nodes — with a framebuffer over each, chunks
accumulated into the edge texture with no clear between them, the dim applied as the alpha of one
untextured quad per texture, and `transferToImageBitmap` called once per settled frame to show the
result rather than once per chunk. It was correct: the backend gate passed all six rows, parity
against Canvas2D was 1.06% of pixels, a lost context still fell back, every measured run reported
`drawnEdges 1999996` of `edges 1999996` with no exception, no console error, no store error and
no banner, and the settled frames are `target/studio-settle/ab-new-1.png` against the reverted
tree's `ab-old-1.png`. It was also slower, so per the job it is reverted and the numbers are the
result.

Interleaved A/B at 1M nodes, `layout.random`, three rounds alternating the two bundles: the six
`webgl2` sources were swapped between the committed text and the GPU-picture text and
`scripts/studio.sh build` rerun before each measurement
(`target/studio-settle/ab-{old,new}-<round>.png`). Milliseconds from the probe's first poll, which
begins once the open command has returned, to the frame whose counters say the picture holds all
1 999 996 edges. The open's own seconds are printed beside it and are not in these.

| Round | kept 2D picture | GPU textures | load 1-min before each |
|---:|---:|---:|---|
| 1 | 28 606 | 83 717 | 19.19 / 19.64 |
| 2 | 36 995 | 77 048 | 38.69 / 27.60 |
| 3 | 41 612 | 66 771 | 29.61 / 27.76 |
| median | **36 995** | **77 048** | |

Both builds took 976–977 frames to fill, so the chunk trajectory was identical; the difference is
the cost of a frame, not of a chunk. A first series, three runs of each build in blocks rather
than interleaved, read 35 834 / 26 105 / 34 480 ms for the kept picture against 37 597 / 39 914 /
44 129 ms for the textures, 1.1x. Both series agree on the sign and neither resolves the magnitude
on this host. A fourth run on the reverted tree as committed, `settle.py 1000000 webgl2 final`,
read 38 076 ms over 977 frames.

## The row that owns the fill

`deploy/perf/settle-profile.py` samples the renderer's main thread once a millisecond over the
fill and groups the samples by call frame. Both runs below are 1M nodes on `layout.random`, taken
back to back on the same host (1-minute load 24.7 before the first and 35.6 after the second),
976 frames each.

| | kept 2D picture | GPU textures |
|---|---:|---:|
| fill | 61 857 ms | 102 554 ms |
| the renderer's own time | 49.1 s | 76.1 s |
| **`transferToImageBitmap`** | **41.5 s (84.6%)** | **70.4 s (92.5%)** |
| `drawImage` | 2.26 s (4.6%) | 0 |
| readback per frame | 42.5 ms | 72.1 ms |

The GPU picture removes every copy the 2D picture made — all of `drawImage`, 2.26 s over the
whole fill — and pays 29 s more for the readback that shows it. What changed for the readback is
who wrote the canvas: two full-canvas textured quad blits through a fragment shader, against the
driver rasterising lines straight into the canvas' own backing store. Reading a shader-written
canvas back costs 1.7x reading a line-drawn one here. *That last sentence is a reading of the
profile, not a measurement of the driver:* the probe can see that the readback got more expensive
and that its input changed, and not why the driver prices it that way. On a hardware driver both
paths are cheap and the balance could go the other way; nothing here can say.

This also corrects what `still.ts`'s header claims: the 99% of a settled frame inside
`transferToImageBitmap` was never the flush of the edge draw that chunking was meant to spread.
It is the readback itself, it is 42.5 ms a frame here whatever drew the canvas, and 977 of them
is the fill.

## What the fill costs

With the moving floor, `frames` was 976–977 in every run of both round-1 builds, over 1 999 996
edge pairs: 2048 pairs a frame, which is `MOVING_FLOOR` (`webgl2/plan.ts:117`).
`nextBudget` halves the chunk every frame because the whole frame costs more than `SLOW_MS`
(24 ms) and floors it at 2048, so the chunk never grows. The fill is then `frames x per-frame
fixed cost` — 977 x 42.5 ms is the 41.5 s of readback the profile above measures — and the draw
calls are not in it: `drawElements` took 28 of the 49 073 samples, and everything that is not
`transferToImageBitmap` or `drawImage` is `(program)`, the rest of the frame.

So the lever on a 1M settle is **the number of readbacks, not the copies**, and round 2 below
pulled it. What the profile could not say was how much of the 42.5 ms is the readback itself and
how much is the rasterising it waits for; the round-2 A/B answers it by changing nothing but the
chunk size. Halving the frames took 26% off the fill, so the fixed per-frame cost — the readback,
the GL clear and the two composites that show the picture — is about a quarter of a 1M fill, and
the other three quarters is drawing two million edges. That is this lever's ceiling: a fill in one
frame would be about 19 s here, not 0.

## Round 2: the still's own floor, and what it bought

Two changes, both in `packages/graph-render`.

`view.stats()` now carries the loop's own `refining` flag (`webgl2/hook.ts:29`, set at
`hook.ts:74` and reported at `view-stats.ts:25`), so a host can wait for a settled picture without
reading counters; `deploy/perf/probes/settle.js` waits on that instead of comparing `drawnEdges`
against `edges`. `refiningOf` (`webgl2/still.ts:59`) is the rule the loop and the stats share, and
`tests/view-stats.test.ts` pins it: over zero while a pair remains, false on the frame that
completes the picture, false on the -1 of a lost context.

The fill's floor moved from the moving one to its own: `stillFloor` (`webgl2/plan.ts:142`, over
`STILL_FRAMES` at `plan.ts:135`) is a 512th of the edge pairs, so the fill takes about 512 frames at
1M however big the graph is, where the moving floor's 2048 pairs (`plan.ts:117`) gave 977.
Interleaved A/B, 1M, three rounds alternating one-line builds — `STILL_FRAMES` 512 against 1e9,
which returns `MOVING_FLOOR` and so is the code exactly as it stood.

| Round | before (977 frames) | after (512 frames) |
|---:|---:|---:|
| 1 | 26 743 | 17 567 |
| 2 | 25 721 | 19 003 |
| 3 | 23 952 | 24 688 |
| median | **25 721** | **19 003** (**−26%**) |

Frame times in the same runs, sampled by the probe every 50 ms: p50 19.6–21.8 ms before against
28.2–43.1 ms after, so a settled frame costs about 1.5x and there are half as many. Round 3 is the
noisy one on both sides (23 952 before against 24 688 after, its p50 the 43.1 ms) and is the run
that says 26% is the median and not the mean. At 200 000 nodes the floor does not bind — 399 996
pairs over 512 is 782, under `MOVING_FLOOR` — and both builds read 196 frames and the same picture,
as the fingerprint below confirms.

**Input still wins.** `deploy/perf/settle-pan.py` waits for a fill a quarter of the way in, pans
with the view's own `panBy`, and reads the gaps in two windows: the pan's own frames (the gaps
ending inside the loop's `MOVING_MS`) and the re-fill that follows. At 1M, one run each:

| | before | after |
|---|---:|---:|
| `panP50GapMs` (6 gaps) | 16.7 | 16.6 |
| `panMaxGapMs` | 16.8 | 16.8 |
| `firstFrameMs` | 31.4 | 23.5 |
| `refillP50GapMs` | 33.3 | 33.4 |
| `refillMaxGapMs` | 316.6 | 349.9 |
| `refiningMoving` | false | false |
| `minDrawnAfter` | 2 048 | 3 907 |

The pan's frames are the same length either side, the first frame answering the pan is not slower,
and `refiningMoving` reads false in both: a camera move owes no chunk, so the still is dropped while
the camera moves and the picture starts again from one chunk (`minDrawnAfter` is the floor in
force — 2048 before, and 3907 after, which is `1 999 996 / 512 = 3907.25` rounded up, so the floor
binding as designed). The refill's p50 is unchanged too, because the page paints at most one frame
per 16.7 ms and a settled frame of 28–43 ms still fits inside one gap; its max rose from 317 to
350 ms, which is the cost of the longer settled frame and the whole of it.

One run each, so read the two 16.7 ms pan figures as "the same length" and not as a difference:
six gaps is a small sample. `p95GapMs` over the whole window did move, 33.4 against 50.1, which is
the refill's longer frames reaching into the tail.

**The pixels.** `settle.js` also fingerprints the canvas (`pixelHash`, an FNV-1a 32 over the
1920x1080 RGBA), because a full-page screenshot differs in the status line between two runs of one
build and cannot say whether the drawing changed. At 200 000, where the floor does not bind, the
two builds are bit-identical: `d3e9cfe9` both. At 1M they are not: `9cb0ed9a` before (twice, in two
runs) against `24a06c31` after (twice, in two runs) — both builds reproduce exactly, so the
difference is systematic, not noise.

So the constraint "`scripts/studio-backend.sh` parity stays at its current threshold" holds — 1.0490%
against a 2% ceiling, unchanged — but the stricter reading, that the pixels are bit-identical, does
not. The mechanism is the chunk boundary, not the drawing: the edges are the same 1 999 996 in the
same spread order with the same colours, but two overlapping edges inside one chunk blend in the GL
pass while the same two edges in two chunks blend in the 2D composite, and those two round
differently. That makes the fingerprint a witness to the chunk boundary and not to a defect in the
drawing, and it means the picture was never specified to be bit-identical across chunk sizes: a
smaller chunk does not make it more correct, it only moves where the rounding happens. **What is
unmeasured here is how far the two pictures differ** — the fingerprint says "not the same bytes" and
not "differ by at most one level in a channel". A build that wanted bit-identical pictures across
chunk sizes would have to accumulate on the GPU, which round 1 measured at 2x slower.

## Texture size limits

`MAX_TEXTURE_SIZE` does not bite on this host: 8192 against a 1920x1080 canvas at DPR 1. The
layer read it beside `ALIASED_POINT_SIZE_RANGE` and `MAX_VIEWPORT_DIMS` and clamped the picture
by the longer side's overflow, so a canvas over the limit is scaled to fit and the quad stretched
it back (NEAREST, so blocky where it clamps). The arithmetic, unit-tested with four cases while
the code existed and not shipped with it: 1920x1080 at a limit of 1024 is a 1024x576 texture, and
at DPR 2 the same canvas wants 3840x2160 and clamps to the same 1024x576. A driver that would
not complete a framebuffer got `null` and painted each frame whole, and a lost context still fell
back to Canvas2D (`webgl2/hook.ts:91`).

## The gates on the round-2 tree

`scripts/orch/gate.sh` on `scripts/orch/rows/perf-p5.rows`, all five rows:

| Row | Expect | Exit | Verdict |
|---|---|---:|---|
| `studio-check` | 0 | 0 | PASS |
| `backend` | 0 | 0 | PASS |
| `negctl-backend` | nonzero | 1 | PASS |
| `smoke` | 0 | 0 | PASS |
| `negctl-smoke` | nonzero | 1 | PASS |

`studio.sh check` (exit 0) covers types over all four tsconfigs, 405 render tests and 533 studio
tests with 0 failed and 0 skipped, 90 render tests, eslint at `--max-warnings 0`, and the
production build. The backend gate (exit 0) passes all six rows — parity at 2k 1.0490% of pixels
off by more than 32/255 against a 2% ceiling, both canvases drawn (44.4785% and 40.9780% off the
background), `auto` picks WebGL2 at 20k, no-WebGL2 falls back to Canvas2D and says why, that page
is clean, and a lost context falls back to Canvas2D and keeps drawing at 44.4798% off the
background. Its negative control (exit 1) reads parity 40.4701% and the WebGL2 canvas 0.1556% off
the background, both FAIL. The smoke gate (exit 0) passes its five rows and its negative control
(exit 1) with the injected wasm module, a store error, the banner and 0 nodes drawn.

Two screenshots of this tree, each with the run's own error read
(`exceptions []`, `console errors []`, `store error None`, no banner in the shadow root):

| What | Path | Status line |
|---|---|---|
| The settled picture at 1M after the fill | `target/studio-settle/r2-after-1.png` | `1000000 n · 1999996 e · idle, last 39 fps · 30 ms · webgl2` |
| The re-fill after the mid-fill pan | `target/studio-settle-pan/r2-pan-after.png` | `1000000 n · 1999996 e · idle, last 47 fps · 62 ms · webgl2` |

Both report all 1 999 996 edges of the frame, so both pictures are full.

## Not reached: the 200k pan gap

The job's round-1 third item, 58.5 to 60 fps at 200 000 nodes, was not measured with a profile, in
either round. The tree's `studio-perf` reads 38.4 worst fps at 200k and at 1M, against 51.1 and
53.5 for the same code hours earlier: 13 fps of spread on one build is larger than the 1.5 fps the
item asks for, so it needs interleaved A/B on a quieter host, or a container with a GPU, before any
row can be named. Round 2's pan probe measures a single pan's frame times rather than a sustained
drag, which ranks builds but is not the fps the item names.

## The three probes

`deploy/perf/settle.py` opens N nodes on `layout.random`, prints the open's own seconds, then
polls from inside the page every 50 ms until the picture is full and prints the milliseconds, the
frame times it sampled, the canvas fingerprint, the counters, the exceptions, the console errors,
the store error and the alert, and writes `target/studio-settle/<label>.png`.
`deploy/perf/settle-profile.py` is the same open and the same wait with `Profiler.start` around it,
and prints the sampled frames by self time as a share of the samples, keeping the raw profile in
`target/studio-settle-profile/<label>.json`. `deploy/perf/settle-pan.py` is the same open, then
waits for a fill a quarter of the way in, pans with the view's own `panBy`, and prints the frame
gaps in the two windows above plus what the picture did afterwards, writing
`target/studio-settle-pan/<label>.png`.

The wait is the loop's own flag, which round 1 had to work around: `refining` was internal to
`webgl2/hook.ts` and `view.stats()` never said it, so the probe compared `drawnEdges` against
`edges`. Round 2 put it in the stats. `ViewStats` has two complete literals outside
`packages/graph-render` (`packages/graph-studio/tests/ui/desk.ts:19` and `tests/ui-names.test.ts:9`),
which the new required field touched; `frameLine` in `packages/graph-studio/src/ui/names.ts` reads
the stats field by field and was left alone, so the studio's status line is unchanged.
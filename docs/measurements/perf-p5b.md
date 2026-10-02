# Perf P5b — the settled picture on the GPU, and what the fill really costs

Measured 2026-10-02 in worktree `perf-p5b`, host dlesieur42 (20 cores, load average 13–38 from
other jobs), image `gm-chromium` (Chrome 154.0.8037.57, viewport 1920x1080, DPR 1). WebGL2 runs
on SwiftShader, Chrome's CPU rasteriser: there is no GPU in the container. Base commit `9725eb5`.

```sh
scripts/studio.sh build
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/settle.py 1000000 webgl2 <label>
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/settle-profile.py 1000000 webgl2 <label>
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

## What the fill costs, and the next lever

`frames` was 976–977 in every run of both builds, over 1 999 996 edge pairs: 2048 pairs a frame,
which is `MOVING_FLOOR` (`webgl2/plan.ts:117`). `nextBudget` halves the chunk every frame because
the whole frame costs more than `SLOW_MS` (24 ms) and floors it at 2048, so the chunk never
grows. The fill is then `frames x per-frame fixed cost` — 977 x 42.5 ms is the 41.5 s of readback
the profile above measures — and the draw calls are not in it: `drawElements` took 28 of the
49 073 samples, and everything that is not `transferToImageBitmap` or `drawImage` is `(program)`,
the rest of the frame.

So the lever on a 1M settle is **the number of readbacks, not the copies**. The profile says the
draw is not where a frame goes — `drawElements` took 28 samples of 49 073 and everything that is
not the readback or a copy is `(program)` at 4 929 — so a settled frame that adds a chunk and
does not show the picture should cost a fraction of a showing frame, and alternating the two would
halve the readbacks and roughly halve the fill, at the price of the picture advancing every other
frame. That last step is a proposal, not a measurement: this job did not time a frame that skips
the readback, and the pacing it would sit inside is a decision in `plan.ts` about how long a
settled frame may be, which is a separate job.

## Texture size limits

`MAX_TEXTURE_SIZE` does not bite on this host: 8192 against a 1920x1080 canvas at DPR 1. The
layer read it beside `ALIASED_POINT_SIZE_RANGE` and `MAX_VIEWPORT_DIMS` and clamped the picture
by the longer side's overflow, so a canvas over the limit is scaled to fit and the quad stretched
it back (NEAREST, so blocky where it clamps). The arithmetic, unit-tested with four cases while
the code existed and not shipped with it: 1920x1080 at a limit of 1024 is a 1024x576 texture, and
at DPR 2 the same canvas wants 3840x2160 and clamps to the same 1024x576. A driver that would
not complete a framebuffer got `null` and painted each frame whole, and a lost context still fell
back to Canvas2D (`webgl2/hook.ts:91`).

## The gates on the reverted tree

`scripts/studio.sh check` passes: types, both unit suites with 0 failed and 0 skipped, 90 render
tests, eslint at `--max-warnings 0`, and the production build. The backend gate passes all six
rows — parity at 2k 1.0490% of pixels off by more than 32/255 against a 2% ceiling, both canvases
drawn (44.4785% and 40.9780% off the background), `auto` picks WebGL2 at 20k, no-WebGL2 falls
back to Canvas2D and says why, that page is clean, and a lost context falls back to Canvas2D and
keeps drawing at 44.4798% off the background. Its negative control exits 1 with parity 40.4701%
and the WebGL2 canvas 0.1556% off the background. The smoke gate passes its five rows and its
negative control exits 1 with the injected wasm module, a store error, the banner and 0 nodes
drawn.

## Not reached: the 200k pan gap

The job's third item, 58.5 to 60 fps at 200 000 nodes, was not measured with a profile. The
reverted tree's `studio-perf` reads 38.4 worst fps at 200k and at 1M, against 51.1 and 53.5 for
the same code hours earlier: 13 fps of spread on one build is larger than the 1.5 fps the item
asks for, so it needs interleaved A/B on a quieter host, or a container with a GPU, before any
row can be named.

## The two probes

`deploy/perf/settle.py` opens N nodes on `layout.random`, prints the open's own seconds, then
polls from inside the page every 50 ms until the picture is full and prints the milliseconds, the
counters, the exceptions, the console errors, the store error and the alert, and writes
`target/studio-settle/<label>.png`. `deploy/perf/settle-profile.py` is the same open and the same
wait with `Profiler.start` around it, and prints the sampled frames by self time as a share of the
samples, keeping the raw profile in `target/studio-settle-profile/<label>.json`.

The page has no "the still is full" flag: the loop keeps `refining` (`webgl2/hook.ts:29`) and
`view.stats()` never says it, so the probe reads `drawnEdges` against `edges` — the pairs the
picture holds and the pairs the frame has (`webgl2/still.ts:107`). That is one frame late at
worst, under the poll interval. Exposing `refining` in `ViewStats` would need a new field on
every `ViewStats` literal in `packages/graph-studio`, which is another package's to change.
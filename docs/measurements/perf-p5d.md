# Perf P5d — the 1M zoom-out cost is GPU edge raster, and what an edge sample of it buys

Measured 2026-10-03 in worktree `perf-p5d` (all changes uncommitted; the orchestrator commits),
image `gm-chromium`, viewport 1920x1080 DPR 1 (`deploy/perf/zoom.py:68`), layout `layout.random`,
host load average 4.1–8.0 during the runs below. Renderer, printed by every probe:

```text
ANGLE (AMD, Vulkan 1.4.305 (AMD Radeon RX 6600 (RADV NAVI23) (0x000073FF)), radv)
```

One round of each arm (r2) ran while other jobs held the machine and read worse in both arms; every
headline number below is a median **including** that round, and the per-round values name it.

## Reproduction

```sh
scripts/studio.sh build                                   # build first; the probes never build
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh zoom 1000000 webgl2 <label>
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh zoom 200000 webgl2 <label>
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh settle-pan 1000000 webgl2 <label>
GM_GPU=1 PERF_MEMORY=10g scripts/studio-perf.sh --cases 1000000 --backend webgl2 \
  --layout layout.random --label <label>                 # exits 1: --cases, gating NOT-RUN
scripts/studio.sh check; scripts/studio-backend.sh; STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh
scripts/studio-smoke.sh; STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh
```

Rounds: `/tmp/opencode/p5d-before-batch.sh`, `/tmp/opencode/p5d-after-batch.sh`, logs
`/tmp/opencode/p5d-{before,after}-logs/`, one log per run plus `batch.log` with every exit code.

## 1. What the cost is (`EXT_disjoint_timer_query_webgl2`)

There was no way to time the edge draw before this job, so the renderer grew one: `EdgeTimer` in
`packages/graph-render/src/webgl2/gputimer.ts` (spec-fixed `TIME_ELAPSED_EXT` 0x88bf and
`GPU_DISJOINT_EXT` 0x88bb; a `SILENT` no-op timer where the extension is absent), bracketing the
`drawElements` at `packages/graph-render/src/webgl2/draw.ts:80`, surfaced as
`ViewStats.gpuEdgeMs` through `hook.ts` and `view-stats.ts`, and diffed per phase by the probe
(`deploy/perf/probes/zoom.js:19`, `:27`). **Caveat:** a timer read of 0 cannot be told apart from
no timer (`zoom.js:38`), and a disjoint query under-reports, so these are lower bounds.

Before-arm round 1 at 1M (the pre-sample build, `p5d-before-1m-r1`):

| phase | fps | p50 / p95 / max gap ms | edges drawn | GPU edge ms per frame |
|---|---:|---|---:|---:|
| zoom in | 54.8 | 16.7 / 16.8 / — | 1 999 996 | **3.15** |
| zoom out | 19.6 | 33.4 / 100 / 100 | 1 999 996 | **49.62** |

The main thread is not the cost: `studio-perf --cases 1000000` reads JS mean **2.17 ms/frame**
(2.169 / 2.165 / 2.714 over three rounds), JS p95 15.6 ms, **0 long tasks** — while a frame costs
49.6 ms of GPU edge time. Same owner in both directions: `draw.ts:80` `drawElements` under
`blendFunc(ONE, ONE_MINUS_SRC_ALPHA)` (`draw.ts:150`) with 1-pixel lines and no depth test, so a
near-fit frame rasterises about 480 chords per pixel over the whole screen. (The zoom-in row's max
gap was not recorded — that run's log predates the batch and this doc's rounds; the other columns
are as printed.)

## 2. Why the fix is a density sample and not the other three

The job's candidates were (a) a sub-pixel edge LOD, (b) less overdraw, (c) a cached still of the
far picture. Step 1 rules (a) and (c) out and leaves (b) with no headroom:

* **(a) never fires.** `TARGET_SPACING = 56` (`frame.ts:10`) rescales every layout so the mean
  spacing is 56 world units, which makes the *mean chord at fit a constant of the viewport*:
  0.52 × 56·√n × (952 / 56·√n) ≈ **496 px** at 200k and at 1M alike. Sub-pixel needs a scale
  below 1/29200 ≈ 0.00003 CSS px per world unit, and `limitsFor` (`camera.ts:78`) floors the
  scale at half the fit scale — 0.0085 at 1M, some 250× above what sub-pixel would need — so no
  zoom-out frame is ever sub-pixel. The zoom-out phase passes through scales 40 → 0.126
  (`zoom.js` sends 60 wheel events of deltaY 60, `WHEEL_ZOOM = 0.0016` ⇒ ×1.10 per step, clamped
  at the limits).
* **(b) has no headroom.** Lines are already one device pixel (WebGL's floor) and blend cannot be
  dropped without changing pixels: the edge colour carries alpha 0.34/0.30 (`theme.ts:40,54`),
  and `draw.ts:150`'s premultiplied blend is what makes crossings composite. Best case a
  write-only pass: ~49.6 → ~25 ms, still short of the 20 ms a frame needs at the cap.
* **(c) cannot cover the phase.** The still at zoom-out time is the picture the *previous* phase
  settled (the probe zooms in to the 40× clamp first, `zoom.py` settles there), not a far view;
  and the out phase spends 35 of its 60 steps at scales 40 → 1.35× fit, where `landing()`
  (`glide.ts`) would refuse a still magnified that far. A far-still path buys the last ~7 steps.

So the cost is moved instead: a moving frame that would cover the screen N times over draws one
edge in `step` of that N, with the alpha scaled so stacked strands read the same.

## 3. The change

* `packages/graph-render/src/webgl2/sample.ts` (new, pure): `measurePairs` (mean chord and
  bounding span over the drawn pairs, a stride over the spread order so any prefix is a spread
  sample), `sampleStep` (coverings = `drawn × mean × scale / (width × height × dpr)`, step =
  `floor(coverings / 24)` clamped to [1, 8], and 1 when the viewport takes under `FILLS = 0.125`
  of the graph's area — i.e. when most chords are off screen and the ratio would lie), and
  `compensate` (the `u_alpha` multiplier that makes `step` sampled edges read as the coverage of
  `step` stacked lines: `(1 − (1 − α)^step) / α`).
* `draw.ts:65` `drawEdges`: the step applies to **moving frames only** (`input.moving`, `draw.ts:69`), divides
  `pace.budget` by it, and scales `u_alpha`. `drawBulk`'s settled fallback (`hook.ts:78`, when
  there is no `OffscreenCanvas`) is not moving and so draws everything.
* `sync.ts:73` `syncEdges(layer, input, placed)`: measures the shape whenever the edge key, the
  positions or the `placed` counter move, and keeps the pair array (`layer.ts:52` `Uploaded.index`)
  so a placement change re-measures without rebuilding the index.
* **The settled picture is untouched**: `hook.ts:95` routes a non-moving frame to `paintStill`,
  whose chunks (`still.ts:94`) draw every pair at full alpha, and `compensate(a, 1) = 1` is the
  identity. So `drawnEdges` reaches `edges` exactly as before and the fill still converges on the
  whole picture.

The arithmetic behind the constants: `step = min(8, floor(o / 24))` with `o` the estimated
coverings leaves at least **24** estimated crossings whatever it picks (for `step < 8`,
`o / step ≥ 24`; for the ceiling, `o ≥ 192 ⇒ o / 8 ≥ 24`). At the theme alpha 0.34, 24 crossings
leave `(1 − 0.34)^24 = 6e-5` of the background — the sample and the full draw are the same image
where they overlap densely, which is everywhere in the band the LOD fires in.

**Ponytail** (the marker's own words, `sample.ts:sampleStep`): the estimate counts every chord in
full even where the viewport clips it, so for a graph up to eight times the screen's area the step
over-thins — wrong direction is *lighter*, never darker; a rare hub-spanning chord inflates the
mean the same way. Failing input: a graph of mostly short edges with a few screen-spanning ones
just inside `FILLS`. Escape hatch: raise `FILLS` toward 1 or `COVER` above 24; zooming in past
`FILLS` already disables it.

## 4. Zoom (`deploy/perf/zoom.py`), 60 wheel events in then out

Median of every round of each arm (3 before, 4 after at 1M); per-round values follow the table,
and the round each arm read worst on is named there.

| nodes | arm | in fps / p50 / gpu ms | out fps / p50 / p95 / gpu ms | edges drawn (in / out) |
|---:|---|---|---|---|
| 1 000 000 | before | 53.2 / 16.7 / 5.18 | 19.6 / 33.4 / 116.6 / 49.62 | 1 999 996 / 1 999 996 |
| 1 000 000 | after | 56.5 / 16.7 / 3.5 | 25.9 / 16.7 / 100 / 36.9 | 1 999 996 or a 2–4× sample / 249 999 |
| 200 000 | before | 60.5 / 16.7 / 0.80 | 59.9 / 16.7 / 16.8 / 3.60 | 399 996 / 399 996 |
| 200 000 | after | 60.5 / 16.7 / 0.77 | 59.9 / 16.7 / 16.8 / 2.70 | 399 996 / **79 999** |

Per-round, 1M zoom-out — fps / p50 / gpu ms per frame: before 19.6/33.4/49.62 (r1), 19.6/33.3/49.63
(r3), 11.4/100/84.32 (r2, the contended round); after 26.9/16.7/34.8 (r1), 24.8/16.7/39.08 (r3),
27.9/16.7/33.46 (r4), 16.5/83.4/60.8 (r2, the contended round). 200k zoom-out: before
60.7/16.7/3.57, 59.8/16.7/3.61, 59.9/16.7/3.60; after 59.9/16.7/2.54, 60.3/16.7/2.70, 59.9/16.7/2.76.

**The rule the job set, applied:** 1M zoom-out p50 **33.4 → 16.7 ms (−50%, over the 3% bar)**
and 200k does not regress — 59.9 → 59.9 fps at the 60 cap with 25% less GPU edge time — so the
change is **kept**. **The ≥50 fps target is not met: the median zoom-out is 25.9 fps (best 27.9).**

Why not more, from the same table: eight times fewer edges (1 999 996 → 249 999) returned only
1.34× less edge-draw GPU time (49.6 → 36.9 ms median) and the out-phase p95 gap stayed at 100 ms.
Splitting the two points linearly, the edge-count-scaling part of that frame is about 14 ms and
about 34 ms does not scale with it — per-frame raster, blend and composite work on the same camera
path, which the sample cannot touch. Raising the ceiling from 8 to 16 could therefore return at
most that 14 ms (to roughly 30 fps) at twice the thinning; the next real lever is the per-frame
path (`hook.ts:63`'s blit and the composite), not the sample.

Zoom-in at 1M is unchanged in the rounds that did not sample (before r1 54.8 and r3 53.2 fps,
after r1 53.1 and r4 53.8 fps, all at p50 16.7): `landing()` glides over the kept picture and the
first fresh frames sit near the fit, where `FILLS` still holds. The two after rounds that did
sample during the in-phase (1 048 576 and 524 288 edges, step 2 and 4) read 59.2 and 60.3 fps —
which is where the in-phase median of 56.5 comes from.

## 5. Settle-pan (`deploy/perf/settle-pan.py`)

| nodes | arm | pan p50 / max gap ms | window max gap ms | full picture after the pan | filledAtMs |
|---:|---|---|---:|---|---:|
| 1 000 000 | before | 16.7 / 16.7 | 50 | 1 999 996 | 111 / 111 / 110 |
| 1 000 000 | after | 16.7 / 16.7 | 50 | 1 999 996 | 121 / 121 / 121 |
| 200 000 | before | 16.7 / 16.7 | 16.8 | 399 996 | 231 / 242 / 242 |
| 200 000 | after | 16.6 / 16.8 | 16.8 | 399 996 | 241 / 232 / 232 |

The drag itself is unchanged to the tick at both sizes, the pan still ends on a full picture, and
the fill restart is +10 ms at 1M — systematic (121 in all three after rounds against 110–111 in all
three before rounds) and small: the pan's moving frames now re-measure the shape once when the
positions move. Nothing else moved.

## 6. Main-thread share (`scripts/studio-perf.sh --cases`, 40/80/40 wheel steps)

Worst fps of the frame probe; JS columns are the page's own per-frame timings.

| nodes | arm | worst fps (r1/r2/r3) | JS mean ms | JS p95 ms | long tasks | edges at the sample |
|---:|---|---|---:|---:|---:|---:|
| 1 000 000 | before | 34.3 / 34.3 / 34.3 | 2.169 / 2.165 / 2.714 | 15.6 / 14.8 / 18.9 | 0 | 1 999 996 |
| 1 000 000 | after | 54.2 / 54.3 / 54.2 | 2.457 / 2.164 / 2.320 | 14.1 / 14.8 / 15.5 | 0 | 1 999 996 |
| 200 000 | before | 60 / 60 / – | 0.373 / 0.392 | 3.2 / 3.3 | 0 | 399 996 |
| 200 000 | after | 60 / 60 / 60 | 0.300 / 0.302 / 0.300 | 2.8 / 2.6 | 0 | 399 996 |

1M worst fps **34.3 → 54.2** with the JS columns inside their own spread (2.17 vs 2.32 ms mean): the
frame got cheaper on the GPU, not on the main thread, which is the same conclusion §1 reaches from
the other side. `drawn edges` here reads the *settled* sample (1 999 996), which is why it does not
move — §4's column is the moving one.

Two of the nine `--cases` invocations could not open the 200k case within the probe's 180 s
budget — one of the three before runs and all three after runs, under contention — so the
after-arm 200k row is three dedicated `--cases 200000` runs and the before-arm 200k row has two.

## 7. Parity: the sample is off at the parity sizes

The parity row is a 2000-node settled view (`deploy/perf/backendrows.py`), and a 2000-node graph at
fit measures **1.3 coverings** — below one step (`tests/webgl2-sample.test.ts`, "a parity-sized
graph keeps the full draw"). Two independent reasons make it inert: the geometry itself never
reaches a step, and the parity picture is drawn by the settled fill path, where `draw.ts:69` gates
the step on `input.moving` and `compensate(a, 1)` is the identity. The gate agrees: **1.0490%** of
pixels differ by more than 32 of 255, the same value `perf-p5b.md:181` recorded — to four decimals,
so no parity pixel moved.

| row | measured | verdict |
|---|---|---|
| `backend-parity-2k` | 1.0490% of pixels differ (ceiling 2%), canvas2d and webgl2 at the same camera | PASS |
| `backend-parity-drawn` | canvas2d 44.4785% / webgl2 40.9780% off the background | PASS |
| `backend-auto-large`, `backend-fallback`, `backend-fallback-clean`, `backend-context-lost` | webgl2 at 20 000 nodes; canvas2d fallback named; no console error; fallback after a lost context | PASS |

## 8. Probe fixes that came with it

* `deploy/perf/probes/frame.js` reports `drawnEdges` per phase and `deploy/perf/rows.py` prints the
  column, so every fps row above carries what the frame drew (§4, §6 — without it the arms invert,
  as `perf-p5c.md:144` warned).
* `deploy/perf/probes/settle.js` hashes the **counters** (`drawnEdges`, `refining`) instead of
  reading the canvas back when `GM_GPU=1` (`settle.py` passes the flag), so the probe's own
  readback is no longer a hardware-arm cost.
* `maxFrameMs` needed no change: `settle.js` sorts the polled `frameMs` samples, so the value is
  the worst frame seen while polling (the reading and its caveat are in §2 of `perf-p5c.md`).

## 9. Exit codes, controls and hazards

| what | command | exit |
|---|---|---|
| 12 before + 13 after probe runs (zoom, settle-pan; 1M/200k) | `/tmp/opencode/p5d-{before,after}-batch.sh` | **0** each |
| 3 before + 3 after `--cases 1000000,200000` runs and 3 after `--cases 200000` runs | `scripts/studio-perf.sh --cases …` | **1** each — documented `--cases` behaviour, gating rows NOT-RUN |
| check (533 unit + 90 render tests, tsc ×4, eslint, production build) | `scripts/studio.sh check` | **0** |
| backend gate | `scripts/studio-backend.sh` | **0** |
| backend negative control | `STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh` | **1** |
| smoke gate | `scripts/studio-smoke.sh` | **0** |
| smoke negative control | `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | **1** |

The gate rows are `scripts/orch/rows/perf-p5.rows`; the parity gates stay on the software arm by
construction (they never set `GM_GPU`).

**Hazard: the host is shared.** One round of each arm (the `r2` rounds above) ran while other jobs
held the machine, and both arms read worse there — before 11.4 fps and after 16.5 fps, with 3.5–4.5
minute `--cases` runs against 47 seconds when the host was quiet. Read the medians, and treat a
single round as evidence of nothing on this host.
# Perf P5c — the perf probes on the host's own rasteriser

Measured 2026-10-02 in worktree `perf-p5c`, host dlesieur42 (20 cores, load average 28–38 from
other jobs all evening), image `gm-chromium` id `aa3f0c1a485a` (Chrome 154.0.8037.92, viewport
1920x1080, DPR 1), commit `acaa0ab`. Two arms, three interleaved rounds per arm per size, round *n*
of one arm run next to round *n* of the other (`/tmp/opencode/measure.sh`, log
`/tmp/opencode/matrix.log`):

* **software** — the default: no device in the container, `--disable-gpu
  --enable-unsafe-swiftshader`.
* **hardware** — `GM_GPU=1`: `--device /dev/dri --group-add 993 --group-add 44`,
  `--ignore-gpu-blocklist --use-angle=vulkan` (`deploy/nav/gpu.py`).

Renderer strings, read from `WEBGL_debug_renderer_info` `UNMASKED_RENDERER_WEBGL` and printed in
every result line of every probe (measured data, paste as measured):

```text
hardware : ANGLE (AMD, Vulkan 1.4.305 (AMD Radeon RX 6600 (RADV NAVI23) (0x000073FF)), radv)
software : ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero) (0x0000C0DE)), SwiftShader driver)
```

`--use-angle=vulkan` was kept because it was measured to reach the device; `gl-egl` also reaches
it and `gl` gets no context at all (`deploy/nav/gpu.py`, at the flags).

**Caveat:** the renderer check proves which *context* the page drew on, not who drew every pixel
of the frame — `WEBGL_debug_renderer_info` names the device ANGLE negotiated while the 2D ground
blit under the layer (`packages/graph-render/src/webgl2/hook.ts:57`) and Chrome's own compositing
can still be software-rasterised, so a string that names hardware does not make the whole frame
hardware. It also cannot see a later context loss.

## Reproduction

```sh
scripts/studio.sh build                                    # build first
# software arm
PERF_MEMORY=10g scripts/studio-probe.sh settle 1000000 webgl2 <label>
# hardware arm
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh settle 1000000 webgl2 <label>
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh settle-profile 1000000 webgl2 <label>
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh open 1000000 webgl2 <label>
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh settle-pan 1000000 webgl2 <label>
GM_GPU=1 PERF_MEMORY=10g scripts/studio-probe.sh zoom 1000000 webgl2 <label>
GM_GPU=1 PERF_MEMORY=10g scripts/studio-perf.sh --cases 1000000 --backend webgl2 \
  --layout layout.random --label <label>                    # exits 1: --cases, gating NOT-RUN
# controls
GM_GPU=1 GM_GPU_BREAK=1 scripts/studio-probe.sh settle 2000 webgl2 negctl   # must exit 2
scripts/studio.sh check; scripts/studio-backend.sh; STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh
scripts/studio-smoke.sh; STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh
```

## 1. `deploy/perf/open.py` — open and the first frame after it

Seconds are the wall of `window.__perf.open` **with the profiler on** (this probe always
profiles); first frame is the first rAF callback arrival after the command returned
(`open.py`, `WATCH_FRAMES`/`FIRST_FRAME`).

| nodes | arm | open s (r1/r2/r3) | median | first frame ms (r1/r2/r3) | median |
|---:|---|---|---:|---|---:|
| 200 000 | software | 3.72 / 3.62 / 3.75 | **3.72** | 135 / 135 / 136 | **135** |
| 200 000 | hardware | 3.45 / 3.58 / 3.55 | **3.55** | 145 / 145 / 139 | **145** |
| 1 000 000 | software | 20.61 / 20.62 / 21.31 | **20.62** | 0 / 0 / 0 | **0** |
| 1 000 000 | hardware | 19.76 / 20.77 / 21.44 | **20.77** | 1 / 0 / 0 | **0** |

The GPU moves the open by −1% at 1M and −5% at 200k: the open is not renderer-bound (§6). The
first-frame column is **not** a paint time — it is the arrival of the next tick, and at 1M a tick
was already pending when the command returned, so it reads 0 on both arms. Only the 200k rows
separate anything, and they do not separate the arms.

## 2. `deploy/perf/settle.py` — time to a full picture

`fullMs` starts at the probe's first poll (after the open returned) and stops at the poll whose
counters say the picture holds every edge. Both arms finished with `drawnEdges` equal to `edges`
and `refining` false in all twelve runs (1 999 996 at 1M, 399 996 at 200k).

| nodes | arm | fullMs (r1/r2/r3) | median | frames | p50 frame ms | last frame ms* |
|---:|---|---|---:|---:|---:|---:|
| 200 000 | software | 8928 / 8056 / 8470 | **8470** | 196 | 35.1 | 94.2 |
| 200 000 | hardware | 301 / 301 / 301 | **301** | 9 | 0.4 | 72.2 |
| 1 000 000 | software | 34041 / 27454 / 27645 | **27645** | 512 | 47.3 | 1010.8 |
| 1 000 000 | hardware | 151 / 150 / 150 | **150** | 10 | 0.2 | 342.4 |

1M: **184x** (27 645 → 150 ms). 200k: **28x**.

\* `maxFrameMs` is the largest polled sample (`settle.js:27` sorts `times`, `:42` takes the last),
but each sample is the studio's `frameMs`, the duration of the last finished frame, polled every
`everyMs`. A frame that began before the window opened is still sampled inside it, which is why it
reads 342 > 150 on the hardware arm. Read it as the worst frame seen while polling, not as a worst
frame inside the fill window. `p50FrameMs` is a true median (`settle.js:23`).

## 3. `deploy/perf/settle-pan.py` — the pan's own frames, and the re-fill it restarts

`panP50GapMs`/`panMaxGapMs` are the gaps whose timestamps fall inside the loop's `MOVING_MS`
(`packages/graph-render/src/canvas2d/loop.ts:24`) — the drag itself. The `p50/max` columns are
every gap in the window, so they carry the re-fill the pan restarted.

| nodes | arm | pan p50 / max ms | window p50 / max ms | drawn after pan (of edges) | refining after |
|---:|---|---|---|---:|---|
| 200 000 | software | 16.7 / 16.7 | 33.4 / 116.7 | 88 064 (of 399 996) | true |
| 200 000 | hardware | 16.7 / 16.7 | 16.7 / 33.3 | 399 996 (of 399 996) | false |
| 1 000 000 | software | 16.7 / 16.7 | 50.0 / 466.6 | 113 303 (of 1 999 996) | true |
| 1 000 000 | hardware | 16.7 / 16.7 | 16.7 / 50.0 | 1 999 996 (of 1 999 996) | false |

Medians of three; the triples for the two headline rows are `panMaxGapMs` 16.7/16.8/16.7 and
16.7/16.7/16.7, `maxGapMs` 466.6/449.9/516.7 and 50/50/50, `drawnAfter`
117210/101582/113303 and 1999996/1999996/1999996.

The drag itself is 16.7 ms on **both** arms at both sizes — the camera work is not the cost. What
separates them is the re-fill the drag restarts: at 1M software's window max is 466.6 ms and the
picture is 5.7% full (and had fallen to 3 907 drawn edges mid-window) when the window ends, while
hardware is full again at 16.7 ms gaps and never fell below 997 083. Latency from `panBy` to the
first answering frame (`firstFrameMs`) is a wash: 25.9 software against 32.1 hardware at 1M,
inside this host's spread — nobody should claim it.

## 4. `deploy/perf/zoom.py` — frame times of a zoom in and back out

Three wheel events, then 60 frames measured. `drawn` is what the probe's last stats sample says
the frame carried — the arms do **not** draw the same picture, so fps rows rank each arm against
its own work.

| nodes | arm | in fps / p50 / p95 / max ms | out fps / p50 / p95 / max ms | drawn (in, out) |
|---:|---|---|---|---|
| 200 000 | software | 43.2 / 16.7 / 50.0 / 66.7 | 31.5 / 33.3 / 66.6 / 66.7 | 399 996 / 399 996 |
| 200 000 | hardware | 60.5 / 16.7 / 16.8 / 33.3 | 60.0 / 16.7 / 16.8 / 33.4 | 399 996 / 399 996 |
| 1 000 000 | software | 33.3 / 33.3 / 50.1 / 116.7 | 31.0 / 33.3 / 66.6 / 83.3 | 2 048 / 2 048 |
| 1 000 000 | hardware | 58.1 / 16.7 / 33.2 / 33.4 | 19.6 / 33.3 / 116.6 / 116.6 | 1 999 996 / 1 999 996 |

The 200k row is the clean win (60 fps at the cap, both directions, whole picture). The 1M row is
the honest one: hardware zooms *in* at 58.1 fps with the whole graph, then zooms *out* at 19.6 fps
because each frame now carries 1 999 996 edges; software reads 31 fps over frames that carried
2 048 of them. Same probe, two different jobs.

## 5. `scripts/studio-perf.sh` — worst fps of the driver's pans

`--cases`, `layout.random`, DPR 1; worst fps is the slowest phase of the frame probe (40/80/40
wheel steps). Exits 1 by design under `--cases` (the gating rows read NOT-RUN).

| nodes | arm | open ms (r1/r2/r3) | worst fps (r1/r2/r3) | median | JS p95 ms | long tasks / longest ms |
|---:|---|---|---|---:|---:|---|
| 200 000 | software | 4268 / 4143 / 3983 | 40.9 / 41.4 / 47.1 | **41.4** | 46.0 | 4 / 60 |
| 200 000 | hardware | 3765 / 3712 / 3646 | 58.6 / 60.0 / 60.0 | **60.0** | 5.2 | 0 / 0 |
| 1 000 000 | software | 19278 / 21549 / 22313 | 40.3 / 42.1 / 25.6 | **40.3** | 75.9 | 16 / 88 |
| 1 000 000 | hardware | 19012 / 20401 / 22334 | 34.3 / 34.3 / 34.3 | **34.3** | 16.7 | 0 / 0 |

At 1M the hardware arm's worst fps reads **below** software's (34.3 against 40.3) while drawing the
whole graph against a sample (§4), with 0 long tasks and a 16.7 ms JS p95 against 16 long tasks
and an 88 ms longest task. A gate on this number alone would read backwards; it needs `drawnEdges`
beside it.

## 6. The hardware arm's profiles at 1M — where the time is

Shares are self time over the sampler's window, three runs for `open`, one run for the two
profiles. Worker shares are of the motor worker's own samples (~20 s), not of the page.

**Open (`open.py`, hardware, r1/r2/r3)** — main thread 92.0 / 92.7 / 92.3% `(idle)`; its largest
non-idle row is the studio's `onmessage` at 1.5 / 1.3 / 1.3%. The work is in the motor worker:

| row | file:line | share (r1/r2/r3) |
|---|---|---|
| `Kt` — the synthetic document, `JSON.stringify` at `:192` | `packages/graph-studio/src/source/synthetic.ts:189` | 9.1 / 8.9 / 9.0 → **9.0%** |
| `Bt` — `applyDegreeWeights` | `packages/graph-studio/src/source/synthetic.ts:166` | 6.9 / 8.5 / 8.5 → **8.5%** |
| indexmap lookup out of the interning arena (indexmap's own `get_i`, reached from `arena.rs:154`) | `crates/graph-core/src/arena.rs:154` | 5.9 / 7.2 / 7.3 → **7.2%** |
| `encode` — `TextEncoder.prototype.encode` | `crates/graph-sdk-js/src/staging.ts:21` | 6.9 / 6.8 / 6.7 → **6.8%** |
| `canonical_json::Parser::value` | `crates/graph-contract/src/canonical_json/parse.rs:68` | 6.1 / – / – → **6.1%** |
| `dlmalloc::Dlmalloc::malloc` (allocator, no repo line) | — | – / 5.5 / 5.4 → **5.4%** |

**The top cost of the open at 1M is the worker's document path**: generating it
(`synthetic.ts:189`), weighing its degrees (`:166`), encoding it to a string for the boundary
(`staging.ts:21`) and parsing it back (`parse.rs:68`) — about 30% of the worker's samples together,
before any layout runs. The generator itself is the probe's fixture; the encode/parse/intern rows
are what any document open pays.

**Settle (`settle-profile.py`, 1M)** — hardware: **151 ms wall, 10 frames, 0.2 s of main-thread
time sampled**, and the top row is `(idle)` **65.1%**. The largest non-idle row is the *probe's
own* readback, `getImageData` at `deploy/perf/probes/settle.js:72` **23.1%** (its caller
`pixelHashOf` at `:67` adds 5.9%); the largest renderer row is `(program)` at 3.0% and four 0.6%
rows of the minified `colormap-CvDbcFrY.js:137`. So the answer to "what costs most in a hardware
settle" is: nothing in the renderer — the fill has no main-thread cost left to find.

Software, same size, same probe: 27 045 ms wall, 22.7 s sampled, `transferToImageBitmap`
(`packages/graph-render/src/webgl2/draw.ts:174`) **82.9%**, `(program)` 10.9%, `drawImage`
(`packages/graph-render/src/webgl2/hook.ts:57`) 5.2%.

**Pan (scratch driver, 1M)** — hardware: 2.8 s sampled, `(idle)` **99.0%**, next row `(program)`
0.7%, then `requestAnimationFrame` 0.1%. There is no top cost to name: the pan's main thread is
idle. Software on the same driver: 2.8 s sampled, `transferToImageBitmap` (`draw.ts:174`) **86.3%**,
`(program)` 5.8%, `(idle)` 5.0%, `drawImage` (`hook.ts:57`) 2.6%.

The pan profile came from `/tmp/opencode/pan-profile.py` (68 lines, scratch — it drives
`settle-pan.js` with `fillPolls 1` under `Profiler.start`), mounted into the container rather than
committed, because the job's file list does not include a new probe. `settle-profile.py` and
`open.py` are the committed ones.

## 7. What to attack next, ranked by what was measured

1. **The readback in every settled frame — `packages/graph-render/src/webgl2/draw.ts:174`
   (whole-frame) and `:188` (chunk): 82.9% of a 1M software settle and 86.3% of a 1M software
   pan, and absent from both hardware top-10s.** On hardware it is already free, so this is the
   software arm's ceiling (CI, SwiftShader, any CPU raster) and nothing else. The lever is the
   *number* of readbacks — `webgl2/plan.ts:117` `MOVING_FLOOR`, `:154` `nextBudget` — which
   `perf-p5b.md:109` names as the lever and `perf-p5b.md:139` measured at −26% for the fill; do
   not reopen the copies question.
2. **The open's document path — `synthetic.ts:189` 9.0% + `synthetic.ts:166` 8.5% + the
   encode/parse/intern trio (`staging.ts:21` 6.8%, `parse.rs:68` 6.1%, `arena.rs:154` 7.2%):
   ≈38% of the motor worker's time at 1M, with the page's main thread 92% idle and the wall
   unchanged by the GPU (20.62 → 20.77 s).** Renderer work cannot move this number; the round
   trip of a 1M-node document as one JSON string can.
3. **The full-picture moving frame at 1M — `webgl2/hook.ts:50` `paintWhole` through
   `draw.ts:174`, budgeted by `webgl2/plan.ts:154`: 19.6 fps and a 116.6 ms p95 zooming out on
   hardware, against 58.1 fps zooming in.** This is now the hardware arm's only visible frame
   cost (the fill is 150 ms, the pan's main thread is 99% idle). Attack the edge draw itself, and
   record `drawnEdges` with any fps number or the comparison inverts (§4, §5).
4. **`studio-perf`'s worst fps as a gate: it needs `drawnEdges` on the row, or hardware loses to
   software while drawing 1 000x more (34.3 against 40.3 at 1M).** Not a code change — a row
   change in `deploy/perf/rows.py`.
5. **The probe's own readback now *is* the hardware profile — `deploy/perf/probes/settle.js:72`
   23.1% of the sampled settle.** Hash the counters (`drawnEdges`, `refining`) instead of the
   canvas bytes when `GM_GPU=1`, so the sampler is not mostly measuring the measurement.
6. Unrelated and pre-existing, both arms: the `MotorWorkerLost` store error on graphs ≤ 2 000
   nodes, where the fill never reports full (`settle 2000` runs still exit 0 through it).

## 8. Exit codes, controls and hazards

| what | command | exit |
|---|---|---|
| 48 probe runs (open, settle, settle-pan, zoom × 2 sizes × 2 arms × 3 rounds) | `/tmp/opencode/matrix.sh` | **0** each |
| 12 perf-gate runs (same matrix) | `scripts/studio-perf.sh --cases …` | **1** each — documented `--cases` behaviour, gating rows NOT-RUN |
| check | `scripts/studio.sh check` | **0** |
| backend gate | `scripts/studio-backend.sh` | **0** |
| backend negative control | `STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh` | **1** |
| smoke gate | `scripts/studio-smoke.sh` | **0** |
| smoke negative control | `STUDIO_SMOKE_BREAK=1 scripts/studio-smoke.sh` | **1** |
| GPU negative control | `GM_GPU=1 GM_GPU_BREAK=1 scripts/studio-probe.sh settle 2000 webgl2 negctl` | **2** — `GM_GPU=1 drew on no hardware renderer: …SwiftShader driver` |
| hardware sanity run | `GM_GPU=1 scripts/studio-probe.sh settle 2000 webgl2 gpu-final` | **0**, renderer line = the hardware string |
| profiles | `settle-profile` ×2, pan profile ×2 | **0** each |

The gate rows are `scripts/orch/rows/perf-p5.rows` and were re-run against this tree after the
last code change (logs `/tmp/opencode/final-*.log`); the parity gates stay on the software arm by
construction — they pass `--cases` nothing and never set `GM_GPU`.

**Hazard: the image tag is shared.** `gm-chromium` is rebuilt by other jobs, and a rebuild from a
Dockerfile without the Mesa packages silently drops the device back to SwiftShader while the tag
name stays. That happened mid-job (21:26); the fix was `deploy/chromium.Dockerfile` rebuilt with
its header's build line, id `aa3f0c1a485a`. If a later hardware run reads the software string,
the refusal at `deploy/nav/gpu.py` exits 2 — that is the check doing its job, not a probe bug.

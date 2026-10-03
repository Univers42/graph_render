# Perf 3D GL — the orbit drag, before and after the WebGL2 3D path

Measured 2026-10-03 on branch `perf-3d-gl`, host dlesieur42, image `gm-chromium` (viewport
1920x1080, DPR 1). Two arms: SwiftShader, Chrome's CPU rasteriser and the only WebGL2 a container
without a device has, and the host's GPU (`GM_GPU=1`, AMD Radeon RX 6600, RADV NAVI23, ANGLE on
Vulkan; `deploy/nav/gpu.py` refuses to report a software renderer as that arm).

```sh
scripts/studio.sh build
scripts/studio-probe.sh orbit3d 20000 layout.basic3d.sphere canvas2d before-sphere-20k
scripts/studio-probe.sh orbit3d 20000 layout.basic3d.sphere webgl2   after-sphere-20k
GM_GPU=1 scripts/studio-probe.sh orbit3d 20000 layout.basic3d.sphere webgl2 gpu-after-sphere-20k
```

The probe (`deploy/perf/orbit3d.py`, in-page half `deploy/perf/probes/orbit3d.js`) opens the
studio's `random` source at N nodes on the layout, then turns the yaw 4 px worth (0.04 rad) per
animation frame for 60 frames through `view.setOrbit`, the call the pointer drag reaches. It reads
back the gap between the frames that answered (p50, p95, max) and the loop's own CPU time per frame
(`view.stats().frameMs`: projection and paint calls, raster excluded on the 2D context).

**Caveat:** one run per case. SwiftShader shares the host's CPU with other jobs, so its numbers rank
two painters on this host and say nothing about a GPU; the GPU numbers are one card's. A gap counts
whatever else the page did in that frame, so one garbage collection shows in max and p95. The gap is
capped below by the display's 16.7 ms frame, so two painters that both fit in a frame tie on it
whatever their cost. `frameMs` is the loop's CPU time: it leaves out the 2D context's raster and
the GPU's own work, so it does not compare the two painters and the decision below reads the gap.

## Before: the Canvas2D 3D painter, SwiftShader (tree as cut from develop)

| Case | Edges | p50 gap | p95 gap | max gap | p50 frameMs | p95 frameMs | Open |
|---|---:|---:|---:|---:|---:|---:|---:|
| sphere 2k | 5 991 | 16.7 | 16.7 | 16.8 | 1.2 | 2.0 | 0.14 s |
| sphere 20k | 39 996 | 83.3 | 100.0 | 100.1 | 12.8 | 13.6 | 0.43 s |
| sphere 200k | 399 996 | 516.6 | 866.7 | 883.2 | 499.8 | 846.3 | 1.96 s |
| spring3d 2k | 5 991 | 16.7 | 16.7 | 16.8 | 1.9 | 2.2 | 0.73 s |
| spring3d 20k | — | not run | | | | | |
| spring3d 200k | — | not run | | | | | |

Milliseconds. Screenshots: `target/studio-orbit3d/before-*.png` (not committed).

spring3d at 20k and 200k is **not run**: at 5 000 nodes or more the studio does not run a force
layout as asked, it scatters the nodes and settles them live on the 2D particle mesh
(`packages/graph-studio/src/motor/settle.ts`, `LIVE_NODES = 5000`, `planRun`). The frame on
screen then has no z column and the probe refuses with "view.orbit() is null". Measuring a 3D
force drawing past that size needs a motor change, out of this job's paths.

## After: the WebGL2 3D layer, SwiftShader (`?backend=webgl2`)

| Case | Edges | p50 gap | p95 gap | max gap | p50 frameMs | p95 frameMs | Open |
|---|---:|---:|---:|---:|---:|---:|---:|
| sphere 2k | 5 991 | 99.9 | 116.7 | 150.0 | 88.1 | 110.0 | 0.24 s |
| sphere 20k | 39 996 | 616.7 | 766.7 | 833.4 | 620.4 | 765.6 | 0.40 s |
| sphere 200k | 399 996 | 5 799.8 | 7 782.9 | 9 533.0 | 5 773.6 | 7 769.4 | 2.34 s |
| spring3d 2k | 5 991 | 66.7 | 83.4 | 100.0 | 70.2 | 78.8 | 0.77 s |

On a CPU rasteriser the layer is 5 to 9 times slower at p95 than the Canvas2D painter it replaces.
Not profiled; the likely cost is the fragment work of one antialiased quad per edge, done on the
CPU, plus the picture's copy onto the 2D canvas.

## Before and after on the GPU (`GM_GPU=1`)

Canvas2D is `?backend=canvas2d` on this branch: the painter is the one measured above, unchanged.

| Case | Elements | canvas2d gap p50 / p95 / max | canvas2d frameMs p50 / p95 | webgl2 gap p50 / p95 / max | webgl2 frameMs p50 / p95 | p95 gap gain |
|---|---:|---|---|---|---|---:|
| sphere 2k | 7 991 | 16.7 / 16.7 / 16.8 | 1.1 / 2.0 | 16.7 / 16.7 / 16.8 | 0.1 / 0.3 | 0% |
| sphere 3k | 11 991 | 16.7 / 16.7 / 16.8 | 2.5 / 2.8 | 16.7 / 16.7 / 16.8 | 0.1 / 0.2 | 0% |
| sphere 5k | 14 996 | 16.7 / 16.8 / 16.8 | 3.4 / 3.9 | 16.7 / 16.7 / 16.8 | 0.1 / 0.3 | 0.6% |
| sphere 10k | 29 996 | 16.7 / 16.7 / 16.8 | 4.7 / 7.4 | 16.7 / 16.7 / 16.8 | 0.1 / 0.2 | 0% |
| sphere 13 336 | 40 004 | 16.7 / 16.8 / 16.8 | 5.4 / 6.6 | 16.7 / 16.8 / 16.8 | 0.1 / 0.2 | 0% |
| sphere 15k | 44 996 | 16.7 / 33.3 / 50.0 | 9.5 / 11.6 | 16.7 / 16.7 / 16.7 | 0.2 / 0.2 | 50% |
| sphere 20k | 59 996 | 16.7 / 33.4 / 66.6 | 8.3 / 13.2 | 16.7 / 16.7 / 16.8 | 0.2 / 0.2 | 50% |
| sphere 200k | 599 996 | 233.4 / 333.3 / 449.9 | 174.4 / 271.8 | 16.7 / 16.7 / 16.8 | 0.1 / 0.2 | 95% |
| spring3d 2k | 7 991 | 16.7 / 16.7 / 16.8 | 2.2 / 3.3 | 16.7 / 16.7 / 16.8 | 0.2 / 0.3 | 0% |

Gain is 1 − webgl2 / canvas2d on the p95 gap. Opening a graph costs the same on both painters
(0.05–1.85 s against 0.06–1.91 s); the layer is made on the first 3D frame.

## The default: `auto` takes the layer from 44 000 elements, on hardware only

The brief's rule: GL is the default only where it is more than 3% faster at p95.

| Where | p95 gap gain | `auto` draws with |
|---|---|---|
| SwiftShader, any size | −400% to −800% (5 to 9 times slower) | Canvas2D |
| GPU, up to 40 004 elements | 0–0.6% (both fit in a frame) | Canvas2D |
| GPU, from 44 996 elements | 50–95% | WebGL2 |

- `SPACE_THRESHOLD = 44_000` (`packages/graph-render/src/webgl2/hook3d.ts`) sits between the last
  tie (40 004) and the first gain (44 996). Its `Caveat:` says it is one card at one size.
- A software rasteriser is told by the renderer's name (`softwareNamed` in `webgl2/layer3d.ts`, the
  names `deploy/nav/gpu.py` uses). `failIfMajorPerformanceCaveat` was tried first and does not do
  it: Chrome 154 with `--enable-unsafe-swiftshader` grants that context on SwiftShader (the gate's
  auto row failed with `backend 'webgl2'` until the name check replaced it).
- Under `auto`, a software renderer is a choice, not a failure: `backendFailure` stays empty.
- `?backend=webgl2` is opt-in and takes the layer at any size on any WebGL2.
- Checked on the GPU, `auto`: sphere 20k (59 996) drew with `webgl2`, p95 gap 16.7; sphere 2k drew
  with `canvas2d`, p95 gap 16.7. On SwiftShader, `auto` at 20k drew with `canvas2d` (gate row below).

## The gate: `scripts/studio-3d-gl.sh`

Parity tolerance, measured on the sphere's 400-node first graph turned by one drag, 1400x900:
the share of pixels that differ between the two painters past a channel gap of 8, 16, 32 and 64 is
1.81%, 0.295%, 0.0367% (463 of 1 260 000) and 0.0102%. The gate uses the backend gate's gap, 32,
and a ceiling of 0.1%, the next round value above 0.0367%. What differs: antialiasing at rims and
edges, sub-1.75 px nodes drawn as discs where Canvas2D fills squares, and edge bundles that GL
blends edge by edge. `perf-3d-gl/parity-canvas2d.png` and `perf-3d-gl/parity-webgl2.png` are the pair.

| Row | Measured (`STUDIO_3D_GL_LABEL=current`) | Verdict |
|---|---|---|
| `gl3d-backend` | canvas2d page `canvas2d`; webgl2 page `webgl2`, 400 of 400 nodes, failure '' | PASS |
| `gl3d-parity` | 0.0367% of pixels past 32/255; both at yaw 1.6 pitch 0.6 distance 2016.4 | PASS |
| `gl3d-drawn` | canvas2d 10.10% off the background, webgl2 10.40% | PASS |
| `gl3d-uniform-only` | one drag: bufferData 0, texImage2D 0, uniformMatrix4fv 42, drawArraysInstanced 42 | PASS |
| `gl3d-clean` | no store error, exception or console error on the webgl2 page | PASS |
| `gl3d-context-lost` | `webgl2` before, `canvas2d` after, failure "the WebGL2 context was lost", 10.26% drawn | PASS |
| `gl3d-fallback` | no WebGL2: `canvas2d`, failure "this browser gives no WebGL2 context on an OffscreenCanvas" | PASS |
| `gl3d-auto-software` | `auto`, 20 000 nodes: `canvas2d`, failure '', renderer SwiftShader | PASS |

`STUDIO_3D_GL_BREAK=1` turns the GL draw calls into no-ops: `gl3d-parity` and `gl3d-drawn` FAIL,
exit 1.

## pw MCP pass

`app/dist` served on 127.0.0.1:5173 (the pw server allows ports 5173–5175 only; a random port was
refused with `ERR_BLOCKED_BY_CLIENT`). `?backend=webgl2`, `layout.run layout.basic3d.sphere`
through the studio's `dispatch`, then one real pointer drag from the canvas to the HUD:

- orbit yaw −4.107, pitch 1.521; backend `webgl2`, failure ''; 400 of 400 nodes, 798 of 798 edges
- store error null, no `.gs-alert` or `role=alert` text, console errors 0: PASS
- 4 console warnings, "GPU stall due to ReadPixels", logged at load before any 3D layout ran and
  absent under `?backend=canvas2d`: they come from the 2D WebGL2 layer, not this one
- screenshot `perf-3d-gl/pw-webgl2-orbit.png`; renderer SwiftShader (the pw container has no GPU)

### Finding: `MotorWorkerLost` after the synthetic open

The 20k and 200k sphere rows of the Before table end with a store error, `MotorWorkerLost` ("the motor worker stopped
answering mid-settle"). It is not caused by the drag: the same error shows on an idle 2k sphere
and on a 20k `layout.random` opened through the probe driver's synthetic source, and not on a plain
`__perf.run` of the same layout. The motor's watchdog (`SILENCE_MS`) fires on a live session that
stays silent. The motor is out of this job's paths; it is listed under "decisions needed". The
gate and the pw pass above use a plain layout run.

After the layer, every SwiftShader webgl2 row ends with it, spring3d 2k included, which had none
before: slow frames on the main thread make the silence longer. On the GPU it shows on canvas2d at
200k only, and on no webgl2 or `auto` row.

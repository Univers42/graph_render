# Perf 3D GL — the orbit drag, before and after the WebGL2 3D path

Measured 2026-10-03 on branch `perf-3d-gl`, host dlesieur42, image `gm-chromium` (viewport
1920x1080, DPR 1). WebGL2 runs on SwiftShader, Chrome's CPU rasteriser: there is no GPU in the
container.

```sh
scripts/studio.sh build
scripts/studio-probe.sh orbit3d 20000 layout.basic3d.sphere canvas2d before-sphere-20k
scripts/studio-probe.sh orbit3d 20000 layout.basic3d.sphere webgl2   after-sphere-20k
```

The probe (`deploy/perf/orbit3d.py`, in-page half `deploy/perf/probes/orbit3d.js`) opens the
studio's `random` source at N nodes on the layout, then turns the yaw 4 px worth (0.04 rad) per
animation frame for 60 frames through `view.setOrbit`, the call the pointer drag reaches. It reads
back the gap between the frames that answered (p50, p95, max) and the loop's own CPU time per frame
(`view.stats().frameMs`: projection and paint calls, raster excluded on the 2D context).

**Caveat:** SwiftShader is a CPU rasteriser sharing the host with other jobs, so these numbers rank
two builds on this host and say nothing about a real GPU. A gap counts whatever else the page did
in that frame, so one garbage collection shows in max and p95.

## Before (Canvas2D 3D painter, tree as cut from develop)

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

### Finding: `MotorWorkerLost` after the synthetic open

The 20k and 200k sphere rows end with a store error, `MotorWorkerLost` ("the motor worker stopped
answering mid-settle"). It is not caused by the drag: the same error shows on an idle 2k sphere
and on a 20k `layout.random` opened through the probe driver's synthetic source, and not on a plain
`__perf.run` of the same layout. The motor's watchdog (`SILENCE_MS`) fires on a live session that
stays silent. The motor is out of this job's paths; it is listed under "decisions needed". The
gate and the pw pass below use a plain layout run.

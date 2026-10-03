# Perf PM-live — a large force graph opens scattered and settles live on the particle mesh

Why: at 400 000 nodes the studio ran the frozen ForceAtlas2 Barnes-Hut layout to completion
inside the open, then threw the result away and started a live session from it. The open took
50 s and nothing moved on screen until it returned.

## Design

`planRun` (`packages/graph-studio/src/motor/settle.ts`) decides per request:

| Request | Nodes | Runs in the open | Reported as | Live session engine |
|---|---|---|---|---|
| a force layout | ≥ `LIVE_NODES` (5 000) | `layout.random` (O(n) scatter) | `layout.force.particle_mesh` | `particle_mesh` |
| a force layout | < `LIVE_NODES` | the layout itself | the layout | `barnes_hut` |
| any other layout | any | the layout itself | the layout | none (frozen) |
| the motor has no force session | any | the layout itself | the layout | none |

The particle mesh is O(n + G log G) a tick, against Barnes-Hut's O(n log n) with a large constant.
The settle is the live session the user watches, so the open returns as soon as the scatter does.

## Browser, 400 000 nodes

`deploy/perf/live-tick.py 400000 webgl2 20` in `gm-chromium` (SwiftShader), one run per row.
"Before" is perf-p5c's build (develop at the time), "after" is this branch.

| | Open | First live frame | Live frames | Frames/s | Median frame gap | Alpha at end of window | Page draws/s |
|---|---|---|---|---|---|---|---|
| before | 50.32 s | none in 20 s | 0 | 0 | — | — | 59.3 |
| after, run 1 | 3.75 s | 351 ms | 172 | 8.72 | 40.3 ms | 0.0049 | 48.5 |
| after, run 2 | 3.78 s | 369 ms | 170 | 8.65 | 38.1 ms | 0.0052 | 47.9 |
| after, run 3 (12 s window) | 3.82 s | 476 ms | 96 | 8.12 | 33.0 ms | 0.0513 | 46.6 |

The open is 13× faster, and the graph moves 0.35–0.48 s after it returns. Alpha reaches 0.005 in
20 s, which is the settle's own stopping point.

A frame is not a tick: the gap deciles of run 2 are `8.6, 9.0, 10.7, 29.3, 38.1, 167.8, 233.9,
238.4, 241.1` ms. The short mode is the frame the loop posts without stepping after a tick overran
its 8 ms budget (`liveLoop.ts`). The long mode, about 235 ms, is the tick itself, so the motor
ticks about 4.3 times a second at 400k in serial wasm.

## Browser, 1 000 000 nodes

`deploy/perf/live-tick.py 1000000 webgl2 30`, after only. Before, the open did not return within
the probe's 180 s limit (`deploy/perf/open.py`, perf-p5c).

| Open | First live frame | Live frames | Frames/s | Median frame gap | Alpha first → last | Page draws/s |
|---|---|---|---|---|---|---|
| 10.46 s | 1137 ms | 86 | 2.95 | 68.9 ms | 0.94 → 0.070 | 42.9 |

The tick is about 700 ms at 1M.

## Where the live tick goes, 400k

`LIVE_PROFILE=1 deploy/perf/live-tick.py 400000 webgl2 20`, the motor worker's self time over the
window:

| Function | Self |
|---|---|
| collide gather | 19.0% |
| fft `line_into` | 15.5% |
| idle | 12.7% |
| collide resolve | 11.7% |
| motion velocity | 6.3% |
| link pass | 6.3% |
| link force | 6.2% |
| deposit | about 5.6% |
| `field_at` | 2.6% |
| collide apply | 1.9% |
| `Snapshot::new` | 1.3% |

The tick is serial wasm. The kernels already threaded natively (perf-p3-pm-fft, perf-p3-gather,
perf-p3-wasm-threads) are the lever; the browser needs cross-origin isolation (perf-p3-coi) and a
threaded live session before they apply.

## Gates

- `scripts/studio.sh check`: rc 0. render 405/405, studio 548/548, ui 90/90, eslint, vite build.
- `scripts/studio-smoke.sh`: rc 0; `STUDIO_SMOKE_BREAK=1`: non-zero.
- `scripts/orch/node-slim.sh npm run sdk:typecheck`: rc 0; the SDK abi test 2/2.
- `cargo test -p graph-wasm -p graph-cli`: 507 passed, 0 failed, 1 ignored (a measurement).

## What it does not do

- It does not make the tick faster. 235 ms at 400k and 700 ms at 1M are the serial wasm cost.
- Below `LIVE_NODES` nothing changes: the requested layout runs and Barnes-Hut settles it.
- The scatter is a start, not a layout: the first frames show noise that the mesh pulls into
  shape. A user who asks for ForceAtlas2 by name past 5 000 nodes gets the mesh's settle and is
  told so by the reported layout id.
- Caveat: one run per row under SwiftShader on a host shared with a running gate. The open and
  the tick move with the load average; the before/after ratio is far outside that noise.

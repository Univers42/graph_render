# Perf P5 — the GPU layer, and 1M nodes that pan and zoom

Measured 2026-10-01 and 2026-10-02 on branch `perf-p5`, host dlesieur42 (20 cores, load
average 10–15 from other jobs), image `gm-chromium` (Chrome 154.0.8037.57, viewport 1920x1080,
DPR 1). WebGL2 runs on SwiftShader, Chrome's CPU rasteriser: there is no GPU in the container.

```sh
scripts/studio.sh build
PERF_MEMORY=10g scripts/studio-perf.sh --label p5final --cases 200000,1000000 --layout layout.random --backend webgl2
scripts/studio-perf.sh --label p5final --cases 20000 --layout layout.random --backend canvas2d
docker run --rm --memory 10g --memory-swap 10g -v "$PWD:/w" -w /w gm-chromium \
  python3 deploy/perf/zoom.py 1000000 webgl2 <label>
scripts/studio-backend.sh                          # the backend gate
STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh   # its negative control: expect non-zero
```

**Caveat:** SwiftShader is a CPU rasteriser sharing the host with other jobs, so these numbers
rank builds on this host and say nothing about a real GPU. The same build read 51.4, 47 and
41.3 fps zooming in on three runs hours apart; every "A against B" below comes from interleaved
runs under the same load. `studio-perf`'s "worst fps" is the slowest pan of its driver, and a
timer-driven probe cannot read above about 60 fps.

## The exit targets (`prompts/perf-plan.md` P5)

`studio-perf --cases`, `layout.random`, worst fps over the driver's pans. "Start" is the first
GPU-layer build (`target/studio-perf/p5-webgl2`, `p5c2d-canvas2d`), "final" is
`target/studio-perf/p5final-*` on the tree `5be9985`/`daa45a0` (same bundle).

| Backend | Nodes | Start fps | Final fps | Open, start → final | Target | Verdict (SwiftShader) |
|---|---:|---:|---:|---:|---|---|
| WebGL2 | 200 000 | 20.5 | 58.5 | 2749 → 2497 ms | ≥ 60 | missed by 1.5 fps |
| WebGL2 | 1 000 000 | 17.3 | 59.3 | 18494 → 15817 ms | ≥ 30 | met |
| Canvas2D | 20 000 | 19.8 | 50.3 | 229 → 291 ms | ≥ 30 | met |

WebGPU is not built (`docs/reports/perf-p5.md` §1).

## What moved the numbers

| Change | Before | After | Evidence |
|---|---|---|---|
| Moving GPU frames draw a paced budget with a 2048 floor | 200k 20.5, 1M 17.3 fps | 47.5, 46.8 fps | `p5-webgl2` → `p5floor-webgl2` |
| Ingest ids through `StringArena`, not `BTreeSet<&str>` | 2.1 s of a 15 s worker profile, open 18 633 ms | open 12 454 ms, 1M 48.4 fps | `crates/graph-wasm/src/ingest/ids.rs`; `p5ingest-webgl2` |
| Ingest paths formatted only on refusal | 11 `format!` per node record, 10 per edge | 0 on success | `crates/graph-wasm/src/ingest/at.rs` |
| Canvas2D moving edge budget paced from the frame gap | 19.8 fps at 20k (fixed budget 16 000) | 55.8 at 2048, 53.7 paced | `p5c2d`, `p5c2d-2048`, `p5c2d-paced`; `canvas2d/pace.ts` |
| Settled GPU frame fills a kept picture in chunks | 3.9–4.5 s per settled frame at 1M, 99 % `transferToImageBitmap` | hover 19–26 ms; first fill 10.5–11.1 s over many frames, hover fill 0.5 s | `webgl2/still.ts` |
| Labels follow the camera between plans from 131 072 nodes | full plan about 7 ms a frame at 1M | about 1 ms (estimated, linear rank scan) | `canvas2d/loop.ts` `FOLLOW_FROM` |
| Moving frames glide over the kept picture | see below | see below | `webgl2/glide.ts` |
| Moving frames draw every on-screen node when they fit the budget | zoomed in on 1M, the budget prefix kept 1 of the 20 nodes on screen | the moving frame shows the settled frame's nodes | `webgl2/draw.ts`; `target/studio-zoom/glide-2/{moving,settled}.png` |

## The glide, interleaved A/B at 1M nodes

`deploy/perf/zoom.py 1000000 webgl2`: 60 wheel events 16 ms apart at the canvas centre, in and
then out. The two bundles differ only in `glide.ts`'s call (`target/dist-glide`,
`target/dist-noglide`), swapped before each run, three rounds, load 10.6–14.9.

| Round | glide in | no glide in | glide out | no glide out |
|---:|---:|---:|---:|---:|
| 1 | 51.2 | 36.6 | 38.3 | 37.6 |
| 2 | 50.9 | 36.5 | 42.6 | 28.2 |
| 3 | 50.8 | 29.8 | 42.0 | 38.9 |
| median | **50.9** | 36.5 | **42.0** | 37.6 |

Frame gap p50 zooming in: 16.7 ms with the glide, 33.3 ms without. No run had an exception, a
console error, a store error or an alert.

## Why the zoom probe uses in-page events

Driving the same zoom through CDP `Input.dispatchMouseEvent` measured the input round-trip: a
build whose moving frames cost half as much read lower there, 28.9 fps against 39.8 from in-page
`WheelEvent`s, both at 1M on WebGL2. `deploy/perf/zoom.py` dispatches the events in the page.

## The backend gate

`scripts/studio-backend.sh` on `5be9985`, 6 rows, all PASS: parity at 2k (1.049 % of pixels
off by more than 32/255, ceiling 2 %), both canvases drawn (44.5 % and 41.0 % off the
background), `auto` picks WebGL2 at 20k, a browser without WebGL2 on an OffscreenCanvas falls
back to Canvas2D and says why, that fallback page has no exception or console error, and a lost
context falls back to Canvas2D and keeps drawing. Its negative control exits 1: parity 40.5 %
and the WebGL2 canvas 0.16 % off the background, both FAIL.

# Studio perf baseline — the first studio, before any repair

Measured 2026-09-29 on develop `0d4f9a7`, host c2r19s2 (16 cores, load average 5 from other
cargo jobs), image `gm-chromium` (Chromium 154.0.8037.57, Debian trixie).

```sh
scripts/studio.sh build
scripts/studio-perf.sh --label baseline-v0 --driver v0 --record-baseline   # exit 1
```

Exit 1 is the expected result: this run is the gate's negative control. A harness that
passed the first studio would not be measuring what the user reported.

**Ponytail:** software raster in a container, on a shared host. The frame rates compare
run to run on this machine. They are not what a desktop browser with a GPU reaches, in
either direction; the studio HUD shows those. Blocked time includes scheduler stalls, so it
over-reports under load.

## What the numbers say

| Finding | Evidence |
|---|---|
| The frame loop never stops | 59.5 animation callbacks/s with nothing moving: the aurora background re-arms itself every frame (`src/core/render/background.ts:115-125`) |
| Layouts freeze the page | every motor call is on the main thread; `layout.packing.circle` blocks 250 ms at 120 nodes and 3.5 s at 500 |
| Drawing is raster-bound, not JS-bound | at 2000 nodes JS is 1.3–1.5 ms a frame (p95 3.9 ms) while the frame rate is 13 fps at DPR 1 and 7 fps at DPR 2 |
| One path per edge | 165 812 `stroke` calls over 40 frames at 2000 nodes (profile below): one `beginPath` + `stroke` for each of 5 991 edges, every frame |
| Haloed text is the dearest call | `strokeText` costs 5.7 µs a call against 0.3 µs for a `stroke` (zoom-in phase: 2 932 calls, 16.6 ms) — JS time only, the raster cost comes on top |

Profile of the 2000-node, DPR 2 case (JS time inside each 2D context call; the clock
reads inflate it, so these are proportions):

| phase | fps | `stroke` | `fill` | `drawImage` | `strokeText` | `fillText` |
|---|---:|---|---|---|---|---|
| zoom-in | 16.1 | 39 257 calls, 11.5 ms | — | 3 585, 5.3 ms | 2 932, 16.6 ms | 2 932, 3.6 ms |
| zoom-out | 11.6 | 180 394, 50.9 ms | 98 451, 27.0 ms | 14 880, 14.3 ms | 4 659, 28.4 ms | 4 659, 4.1 ms |
| zoom-back | 8.1 | 165 812, 51.4 ms | 46 360, 14.6 ms | 12 541, 16.8 ms | 3 120, 6.5 ms | 3 120, 4.2 ms |

Not measured here: 20 000 nodes (the first studio refuses more than 2 000), the cost of
the per-frame allocations in `app/src/core/frame.ts`, and anything on a GPU.

## Harness output

### studio-perf — baseline-v0

commit `0d4f9a7` · driver `v0` · Chrome/154.0.8037.57 · viewport 1920x1080 · software raster

| row | expectation | measured | verdict |
|---|---|---|---|
| `perf-idle` | 0 animation callbacks/s when parked | 59.5/s | FAIL |
| `perf-block` | main thread blocked <= 50 ms by any layout | 3467.9 ms (layout.packing.circle at 500 nodes) | FAIL |
| `perf-js` | p95 JS per frame <= 4 ms at 2000 nodes | 3.9 ms | PASS |
| `perf-fps` | >= 2x baseline at DPR 2 (or the 54 fps cap) | no baseline file | NOT-RUN |
| `perf-20k` | recorded, not gated | DPR 1: driver caps at 2000 nodes; DPR 2: driver caps at 2000 nodes | RECORDED |

| nodes | DPR | canvas | worst fps | JS mean ms | JS p95 ms |
|---:|---:|---|---:|---:|---:|
| 120 | 1 | 1552x925 | 59.5 | 0.455 | 2 |
| 120 | 2 | 3104x1850 | 54.1 | 0.425 | 1.7 |
| 2000 | 1 | 1552x925 | 13.3 | 1.497 | 3.9 |
| 2000 | 2 | 3104x1850 | 7 | 1.292 | 3.6 |
| 20000 | 1 | not run: driver caps at 2000 nodes | | | |
| 20000 | 2 | not run: driver caps at 2000 nodes | | | |

| nodes | layout | wall ms | blocked ms |
|---:|---|---:|---:|
| 120 | `layout.grid` | 87.3 | 4.8 |
| 120 | `layout.tree.tidy` | 82.8 | 20.4 |
| 120 | `layout.treemap.squarified` | 83.9 | 7.4 |
| 120 | `layout.circular.radial` | 82.7 | 6 |
| 120 | `layout.packing.circle` | 300.1 | 249.5 |
| 120 | `layout.spectral` | 100.1 | 25.8 |
| 120 | `layout.mds.pivot` | 83.2 | 18.2 |
| 120 | `layout.force.barnes_hut` | 100.3 | 39.3 |
| 120 | `layout.forceatlas2` | 83.4 | 5.7 |
| 120 | `layout.dag.sugiyama` | 99.6 | 24.8 |
| 500 | `layout.grid` | 90 | 30.2 |
| 500 | `layout.tree.tidy` | 98.5 | 27.4 |
| 500 | `layout.treemap.squarified` | 135.3 | 46.8 |
| 500 | `layout.circular.radial` | 81.4 | 31.5 |
| 500 | `layout.packing.circle` | 3522 | 3467.9 |
| 500 | `layout.spectral` | 111.3 | 42.1 |
| 500 | `layout.mds.pivot` | 100.3 | 37.6 |
| 500 | `layout.force.barnes_hut` | 199.8 | 113.6 |
| 500 | `layout.forceatlas2` | 133.3 | 85.8 |
| 500 | `layout.dag.sugiyama` | 117.4 | 54.8 |

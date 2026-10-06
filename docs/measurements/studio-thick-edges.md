# Thick edge strokes and `perf-fps` (2026-10-06)

`perf-fps` had never passed (`docs/measurements/studio-s7.md`). At 2000 nodes and DPR 2 the
zoom-in phase drew 5.3 fps, while JS took 1.2 ms a frame. The time went to rasterising the edge
stroke. It now passes: 24.1 fps against a floor of 14.

## Where the frame went

`scripts/studio-probe.sh noop-zoom 2000 2` zooms 40 wheel steps in and 40 out, at 2000 nodes, DPR 2,
on Canvas2D. For each run, one context method is replaced by a no-op. The build is the tree before
the fix (`dda2d1f2`), on Chromium 154 with SwiftShader:

| replaced | zoom in | zoom out |
|---|---:|---:|
| nothing | 5.3 fps, 18 long tasks, longest 665 ms | 5 fps |
| `stroke` | 62.6 fps, no long task | 60 fps |
| `fill` | 5.6 fps | 16.3 fps |
| `drawImage` | 5.5 fps | 7.1 fps |
| `perEdge`: a stroke after each thick `lineTo` | 32.5 fps, longest task 53 ms | 32.2 fps |

The edges are the cost, and only once zoomed in. `edgeWidth` keeps a stroke one device pixel wide at
low zoom and widens it to 1.5 CSS px (3 device px at DPR 2) once the zoom is close. Skia draws a
stroke of at most one device pixel as a hairline. A wider one is outlined and scan-converted as one
path.

## One path or one stroke per segment

`scripts/studio-probe.sh stroke-batch` draws 1242 seeded segments, each up to 5760 px long, on a
blank 3840x2160 canvas. Each figure is the mean of 6 frames after 2 warm-up frames:

| device width | one path (CHUNK 2048) | 16 per stroke | 4 per stroke | 1 per stroke |
|---:|---:|---:|---:|---:|
| (clear only) | 15.4 ms | | | |
| 1 | 43.7 ms | 33.1 ms | 34.2 ms | 32.6 ms |
| 1.2 | 438.8 ms | 84.5 ms | 78.7 ms | 67.5 ms |
| 1.5 | 408 ms | 87.5 ms | 82.4 ms | 70.2 ms |
| 2 | 352 ms | 90.1 ms | 88 ms | 66.4 ms |
| 3 | 238 ms | 92.5 ms | 79.3 ms | 84.2 ms |

For a hairline, the batch size moves the frame by about 10 ms either way: an earlier run read
43.8 ms for one path and 47.7 ms for one stroke per segment. Past one device pixel, one stroke per
segment is 3 to 6 times faster than one shared path. So `edges.ts` `chunkOf` keeps CHUNK for a
hairline and strokes each edge on its own when the stroke is wider. `perf-edge-batch` still reads 3 strokes at 2000
nodes, because its stats probe measures the opening view, where the stroke is a hairline.

What it changes on screen: the theme's edge colour is translucent (alpha 0.34 dark, 0.30 light). One
shared path covered a crossing once. Now each thick edge is its own stroke, so a crossing is drawn
twice and reads darker (about 0.56 for two dark-theme edges). A hairline is drawn as before.

## The gate, before and after

`scripts/studio-perf.sh`, run alone, on the same host and in the same hour. Before is `dda2d1f2`
(label `base`); after is this change (label `after`):

| case | before | after |
|---|---:|---:|
| `perf-fps` 120 nodes, DPR 2 (floor 54) | 47.7 fps | 60 fps |
| `perf-fps` 2000 nodes, DPR 2 (floor 14) | 5.3 fps | 24.1 fps |
| 2000 nodes, DPR 1, worst phase | 19.1 fps | 57.1 fps |
| 2000 nodes, DPR 2, longest task | 778 ms | 121 ms |
| gate exit | 1 (`perf-fps` FAIL) | 0 (every row PASS) |

Its negative control, `STUDIO_PERF_BREAK=1 scripts/studio-perf.sh --label negctl`, exits 1 on the
fixed tree, with `perf-edge-batch` FAIL. The unit test `paint.test.ts` ("an edge stroke wider than a
device pixel is one stroke per edge; a hairline is chunked") fails without `chunkOf`: 1 stroke
where 570 edges were drawn.

The 10 000-node cases draw on the WebGL2 layer, which never calls the 2D edge painter without a
focus (`canvas2d/paint.ts`). Their 54 → 50.1 fps and p95 JS 24 → 83 ms at DPR 1 come from frames that
were already up to 98 ms in the before run. That row is recorded, not gated.

Caveat: a software rasteriser on a shared host. A GPU rasteriser may prefer the shared path. That
is unmeasured here; `GM_GPU=1 scripts/studio-perf.sh` is the arm that would show it.

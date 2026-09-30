# studio-s7 perf table

Measured 2026-09-30 on host dlesieur42 (20 cores, load average 3 to 5 from peer sessions),
`scripts/studio-perf.sh --label s7`, driver `hook`, Chrome 154, 1920x1080, software raster.
Baseline: `deploy/perf/baseline.json` (see `studio-perf-baseline.md`). The tree is the
studio-s7 branch after the two fixes below.

| row | expectation | measured | verdict |
|---|---|---|---|
| perf-idle | 0 animation callbacks/s when parked | 0/s | PASS |
| perf-block | main thread blocked <= 50 ms by any layout | 10.5 ms (`layout.packing.circle`, 120 nodes) | PASS |
| perf-js | p95 JS per frame <= 4 ms at 2000 nodes | 4 ms (3.5 ms on the previous run) | PASS, at the ceiling |
| perf-fps | >= 2x baseline at DPR 2, or the 54 fps cap | 120 nodes 45.5 fps (floor 54); 2000 nodes 2.4 fps (floor 14) | FAIL, see below |
| perf-edge-batch | stroke() calls <= edge styles + edges / 2048; arrow fills <= 1 | 2000 nodes: 3 strokes, 1 style, 5991 edges (budget 3); 10000 nodes: 10 strokes, 1 style, 19996 edges (budget 10) | PASS |
| perf-sprite-cache | 0 sprites rasterised on a second identical frame | 0 at 2000 and 10000 nodes | PASS |
| perf-label-layout | 0 layout runs over a redraw and a parked frame, >= 1 after a zoom | 2000 nodes: 0 / 0 / 3; 10000 nodes: 0 / 0 / 1 | PASS |
| perf-10k | 10000 nodes / 19996 edges pan/zoom, recorded not gated | DPR 1: 3.8 fps, p95 JS 3.7 ms; DPR 2: 1.1 fps, p95 JS 3.9 ms | RECORDED |
| negative control | `STUDIO_PERF_BREAK=1 scripts/studio-perf.sh` exits non-zero | exit 1; perf-edge-batch FAILs at budget 2 (2000 nodes) and 9 (10000 nodes) against 3 and 10 strokes. perf-fps is red either way, so the exit code alone proves nothing; the row does | PASS |

## perf-fps is red on develop too

develop at 09442bb, same host, same hour, with the only harness change needed to run it (its
20000-node frame case times out; this branch replaced it with 10000):

| case | develop | s7, one path per style | s7, chunked (landed) |
|---|---|---|---|
| 120 nodes, DPR 2, worst phase | 45.1 fps | 44.3 fps | 45.5 fps |
| 2000 nodes, DPR 1, worst phase | 7.1 fps | 5.1 fps | not compared |
| 2000 nodes, DPR 2, worst phase | 2.4 fps | 1.4 fps | 2.4 fps |

The row has never passed: it was NOT-RUN in `studio-perf-baseline.md` and `phase-g0.md`. The
2x target is not met on this host by develop or by this branch; the branch does not move it.

## Two fixes found by the gate

- One path per edge style (no chunk cap) cost 42 % of the frame rate at 2000 nodes, DPR 2,
  with JS time unchanged: the time went to rasterising one unbounded path. `edges.ts` keeps
  the 2048-segment `CHUNK`, and perf-edge-batch now allows `ceil(edges / 2048)` strokes per style.
- A frame that baked a label sprite had laid that label out at width 0 and then scheduled no
  further frame, so the next redraw ran the label layout (redraw 1 at 2000 nodes). `loop.ts`
  now paints one more frame after any bake.

Ponytail: the counter rows are exact counts; the fps and 10k timings are a shared,
software-rastered host and move with its load (perf-js read 3.5 ms and 4 ms on two runs).

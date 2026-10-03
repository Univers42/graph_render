# Job perf-p5d (agent build; 1M nodes fast and interactive, on the real GPU)

Goal: 1M nodes, fast and interactive. Source: `docs/measurements/perf-p5c.md` (read §4, §5, §6, §7
whole). This branch starts from perf-p5c, so its `GM_GPU=1` arm (`scripts/orch/gpu.sh`,
`deploy/nav/gpu.py`) is here. The parity gates stay on SwiftShader; never set `GM_GPU` in a gate row.

Facts measured by perf-p5c (hardware arm, AMD RX 6600, 3 interleaved rounds):
- Zoom at 1M: in 58.1 fps, p95 33.2 ms; **out 19.6 fps, p50 33.3, p95 116.6 ms**, 1 999 996 edges drawn
  both ways. 200k: 60 fps both ways. The page's main thread is idle (JS p95 16.7 ms, 0 long tasks), so the
  zoom-out cost is most likely on the GPU: 2M blended lines over few pixels. That is a hypothesis; step 1 settles it.
- `deploy/perf/probes/settle.js:42` reports `maxFrameMs` as `times[times.length - 1]`, the last
  frame, not the largest.
- `scripts/studio-perf.sh` worst fps reads hardware 34.3 below software 40.3 at 1M while hardware draws
  1000x more edges: the row lacks `drawnEdges` (`deploy/perf/rows.py`).
- Under `GM_GPU=1` the probe's own `getImageData` (`settle.js:72`, `pixelHashOf` `:67`) is 29% of the
  sampled settle.

Do, in order:
1. **Locate the zoom-out cost.** Time the edge draw with `EXT_disjoint_timer_query_webgl2` under
   `GM_GPU=1` (probe-side, or behind a stats field that costs nothing when off), zoom in vs zoom out at 1M,
   3 rounds. Paste GPU ms per frame and the main-thread share. Name the file:line that owns the cost.
2. **Fix it at that file:line** (`packages/graph-render/src/webgl2/`). Candidates, pick by step 1: an
   edge level of detail when edges are shorter than a pixel on screen (skip or sample them with the
   alpha compensated so density reads the same), less overdraw (blend state, line width), or a cached
   still of the far picture reused while zooming. Every heuristic carries a `Ponytail:` line naming
   what it gets wrong. `scripts/studio-backend.sh` pixel parity must stay green unchanged: if the LOD
   would move a parity pixel, it must stay off at the parity sizes, and say so.
3. **Probe fixes**, each with a unit test or a run that shows the change:
   (a) `settle.js` `maxFrameMs` is the real maximum (no spread over a large array);
   (b) `rows.py` and the studio-perf report carry `drawnEdges` beside worst fps;
   (c) under `GM_GPU=1` the settle probe hashes the counters (`drawnEdges`, `refining`, frames)
       instead of reading the canvas back; the software arm keeps the pixel hash.
4. **Measure**: `deploy/perf/zoom.py` and `settle-pan.py` at 200k and 1M, hardware arm, 3 rounds before
   and 3 after, interleaved, `drawnEdges` on every row. Keep the change only if zoom-out p50 drops by
   more than 3% at 1M and 200k does not regress. Target: zoom-out ≥ 50 fps at 1M. Append a section to
   `docs/measurements/perf-p5c.md` (or a new `perf-p5d.md`) with the tables and the commands.

Out of bounds: `crates/`, the motor worker, the open path (open-core-slot and fix-wasm-ingest own it),
`packages/graph-studio/src/motor/` (fix-worker-lost owns it). No new dependency. House limits: 40 lines a
function, 300 a file, 4 parameters, no new TS type assertion.

Done when: `scripts/orch/rows/perf-p5.rows` green, `scripts/studio.sh check` 0, `scripts/studio-backend.sh`
0 and its `STUDIO_BACKEND_BREAK=1` negctl non-zero, `scripts/studio-smoke.sh` 0 and its negctl non-zero,
the measurement committed. If the win is under 3%, revert the renderer change and commit the probe fixes
and the numbers: the numbers are the result.

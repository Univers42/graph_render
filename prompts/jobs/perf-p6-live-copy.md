# Job perf-p6-live-copy (agent build: a live frame at 1M copies and uploads only what changed)

Goal: no per-frame O(n) allocation on the main thread, and no torn reads.

Facts:
- packages/graph-studio/src/motor/liveLoop.ts:97 and :143 build `{ xs: xs.slice(), ys: ys.slice(), ... }`
  every frame (verified). That is two O(n) copies, about 16 MB per frame at 1M.
- Audit cites, to confirm before editing:
  - canvas2d/controller.ts:231-251 and drag.ts:16-27 run O(n) passes on every setPositions;
  - webgl2/sync.ts:12-15 re-uploads the moving columns (docs/reports/perf-p5.md:22: changed columns only,
    which in a live frame means both).

Do, in order:
1. Measure the per-frame main-thread and worker cost of a live frame at 1M: performance marks,
   3 rounds, hardware arm.
2. Hand positions over without copying:
   - Transfer them from a pool of 3 buffers owned by the loop, returned after the draw.
   - When crossOriginIsolated, use one SAB plus a frame-sequence word (Atomics).
   - Test with a deliberately slow consumer: no torn reads.
3. Convert to f32 in the worker, and `bufferSubData` into a preallocated buffer.
4. In setPositions, compute the bounds and the drag index on demand, not every frame.
5. Keep only changes that are > 3% faster. The studio-backend pixel parity stays unchanged.

Out of bounds: crates/, pipeline.ts (perf-p6-data-path), and `src/motor/{session,settle,live,bridge}.ts`
(perf-pm-live owns them until it lands). No new dependency. House limits: 40 lines a function,
300 a file, 4 parameters.

Done when:
- studio.sh check exits 0.
- studio-backend and studio-smoke exit 0, each with its negative control non-zero.
- perf-p5.rows and perf-p6.rows are green.
- docs/measurements/perf-p6-live-copy.md is written.

# Job perf-transition (agent build: a layout switch starts moving at once and costs no CPU per frame)

Why (user, 2026-10-03): "we want even faster, especially in loading or for transition of nodes".

Starts after `perf-p6-live-copy` and `perf-open-path` land; it builds on both. Load time itself is
those jobs' work.

Facts (confirm on the merged tree):
- `packages/graph-render/src/canvas2d/loop.ts:121-135` (`advance`) runs every frame while a
  transition is in flight:
  - `blend()` runs on both columns, O(n) on the CPU (`transition.ts`, `TRANSITION_MS`,
    `easeInOutCubic`);
  - then `state.bulk.placed += 1`, which makes the WebGL2 layer re-upload every position.

  So a transition costs two O(n) passes and one full upload per frame.

Do, in order:
1. Measure the path of a layout switch at 2 000, 20 000 and 200 000 nodes, on both backends, with
   `deploy/perf/cdp.py`. Time these points:
   - the click;
   - the worker request;
   - the bytes arriving;
   - the first frame that moves;
   - the settled frame.

   Also take the frame time p50 and p95 during the tween. Record the table in
   `docs/measurements/perf-transition.md`.
2. Fix the dominant cost the measurement names. The expected fix, if the numbers agree:
   - On WebGL2, upload the `from` and `to` columns once and mix them in the vertex shader with an
     `eased` uniform. No per-frame CPU blend and no per-frame upload.
   - Start the tween on the first column that arrives, not after the whole frame is decoded.
   - Canvas2D keeps the CPU blend under a node budget. Above the budget it snaps, with a
     `Ponytail:` line naming the budget and how it was measured.
3. Picking, labels and edges must read the right positions mid-tween. Hit-testing reads the eased
   position, not the target.
4. Measure again. Keep a change only where it is more than 3% faster.

Paths:
- `packages/graph-render/src/{transition.ts,canvas2d/loop.ts,webgl2/**}`
- `packages/graph-studio/src/studio/` (only for timing marks)
- `deploy/perf/`
- `docs/measurements/perf-transition.md`

Out of bounds:
- `crates/`
- `packages/graph-studio/src/motor/**`
- `three/**` (perf-3d-gl)

No new dependency. House limits.

Done when:
- `scripts/studio.sh check` exits 0, and `perf-p5.rows` is green.
- `scripts/studio-backend.sh` and `scripts/studio-interact.sh` exit 0, and their negative controls
  exit non-zero.
- A pw MCP pass on the built studio is done: switch layouts three times on a 20 000-node source,
  with a screenshot mid-tween and one settled, saved under `docs/measurements/perf-transition/`.
  Read the store error, the console exceptions and the overlay text. Any one of them non-empty is a
  FAIL.
- The measurement table is filled in.

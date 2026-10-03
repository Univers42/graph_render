# Job perf-transition-pick (agent build: a moving node can be clicked, focused and labelled where it is drawn)

Why: `perf-transition` (2026-10-03) left hit-testing off for the whole tween and `focus()` aiming at the
target pose, because the files were outside its paths. Its recommended answer to both was "yes", and it
is adopted.

Your worktree is cut from `perf-transition`, which is not on develop yet; do not merge develop.

Facts (confirm on your tree; line numbers are from perf-transition's report):
- `packages/graph-render/src/canvas2d/controller.ts:248` turns picking off while a transition runs.
- `packages/graph-render/src/scene.ts:108` (`pickIn`) reads the target frame, not the eased pose.
- `packages/graph-render/src/camera-api.ts:125` (`focus()`) pans to the target position mid-tween.
- `packages/graph-render/src/canvas2d/layout-key.ts:54`: `travelling ||` forces a full `planLabels`
  every tween frame. perf-transition measured it as not dominant.
- `state.x` / `state.y` hold the eased pose on Canvas2D. On WebGL2 the eased pose is mixed in the
  vertex shader (`u_eased`), so the CPU side has `from`, `to` and the eased scalar, not a column.

Do, in order (TDD: each step starts with a red test under `packages/graph-render/tests/`):
1. Hit-testing reads the eased pose on both backends. The pick grid indexes the target positions,
   so it cannot find a node by where it is drawn mid-tween. Mid-tween, pick with one linear scan
   over the eased positions (computed per node from `from`, `to` and the eased scalar, no column
   allocated). Measure one pick at 200 000 nodes and record it; hover runs this at most once per
   frame. Drop the `controller.ts:248` guard. The settled path keeps the grid.
2. `focus()` mid-tween aims at the position the node will settle at, and the camera tween ends on it.
   State the choice in one line of the measurement doc.
3. Labels: measure `planLabels` per tween frame at 20 000 nodes on Canvas2D. Change `layout-key.ts`
   only if it is more than 3% of the frame time.

Paths:
- `packages/graph-render/src/{scene.ts,camera-api.ts,transition.ts}`
- `packages/graph-render/src/canvas2d/**`
- `packages/graph-render/src/webgl2/**` (read the eased scalar; no shader change)
- `packages/graph-render/tests/**`
- `docs/measurements/perf-transition-pick.md`

Out of bounds:
- `crates/`
- `packages/graph-studio/src/motor/**`
- the snapshot wire format

No new dependency. House limits.

Done when:
- `scripts/studio.sh check` exits 0, and `perf-p5.rows` is green.
- `scripts/studio-interact.sh` exits 0, and `STUDIO_INTERACT_BREAK=1` exits non-zero.
- A new test clicks a node at 50% of a tween on each backend and gets that node's id. The same test
  fails with the `controller.ts:248` guard restored.
- A pw MCP pass on the built studio: switch layouts on a 2 000-node source, click a moving node
  mid-tween, and screenshot the selection. Read the store error, the console exceptions and the
  overlay text. Any one of them non-empty is a FAIL.

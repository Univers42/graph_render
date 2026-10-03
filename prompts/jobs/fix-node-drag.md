# Job fix-node-drag (agent build: a dragged node follows the pointer again)

Why: dragging a node is the studio's basic gesture, and on develop it does not work.

Facts (measured 2026-10-03 by ux-forces-full; confirm first):
- `scripts/studio-interact.sh`, row `int-node-drag` (`deploy/nav/interactrows.py:132`): after a
  150 px drag, the node ends 145.89 px from the pointer on origin/develop's own build. So the node
  hardly moved, or it moved back.
- The other interact rows pass. `int-box` once failed and then passed on the same build, on an
  edge-node pick.
- Recent commits that touch the drag path: `git log -- packages/graph-studio/src/element.ts
  packages/graph-studio/src/motor/bridge.ts packages/graph-studio/src/motor/session.ts
  packages/graph-studio/src/motor/liveLoop.ts` (acf1c47, e5de232, d4ae3e6, c6d3772, 69147bc, ...).

Do, in order:
1. Reproduce: `scripts/studio.sh wasm && scripts/studio.sh build && scripts/studio-interact.sh`.
2. Find the commit that broke the row. Bisect with that command, building each step. Name the
   commit and the line.
3. Fix the root cause at the shared function, not in the probe. The probe's tolerance
   (`FOLLOW_TOLERANCE`) does not move.
4. Add one unit test in `packages/graph-studio` that fails before the fix. If the bug only shows in
   a browser, the interact row is the test; say so.

Paths: `packages/graph-studio/src/**` and its tests, plus `packages/graph-render/src/**` only if the
pointer path lives there.

Out of bounds:
- `crates/`
- `deploy/nav/interact*.py`, except to add detail to a message

No new dependency. House limits: 40 lines per function, 300 lines per file, 4 parameters.

Done when:
- `scripts/orch/rows/ux-forces.rows` is green. Its rows are `studio.sh check`, backend, smoke, live
  and interact, and each negative control exits non-zero.
- A pw MCP pass on the built studio is done: drag a node 150 px, and save a before and an after
  screenshot under `docs/measurements/fix-node-drag/`. Read the store error, the console exceptions
  and the overlay text. Any one of them non-empty is a FAIL.
- The return block names the breaking commit.

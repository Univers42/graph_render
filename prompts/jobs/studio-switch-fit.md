# Job studio-switch-fit (agent build, studio defect)

Why: measured 2026-10-01 with the pw MCP on the dev studio (127.0.0.1:5174), fixture
`hierarchy/tree-balanced.json`: after clicking layout `force.drl` in the Layout list, the canvas has 0 node
pixels for 6 s; pressing the fit button (⤢) then shows 11958. Native `graph-cli snapshot --layout force.drl`
coordinates are finite (seed 13: x 11.06..13.69, y 11.34..13.51), so the drawing exists and the camera is
somewhere else. In a 34-layout tour on `force/clustered.json`, FR, KK, Davidson-Harel, LGL, spring and fdp
were drawn clipped at the frame edge after ⤢. `setFrame` (`packages/graph-render/src/view.ts:176`) fits
after `showFrame`; with `animate` the transition starts from the old positions
(`canvas2d/controller.ts:123-141`).

Do:
1. Reproduce first, as a failing check (RED): a render test or a `studio-nav.sh` row that switches layout
   and requires every node of the new frame inside the viewport once the transition ends.
2. Find the cause (camera fitted to the wrong bounds, a later `setPositions` from the live force loop, or
   a fit computed before the transition target is known) and fix it at the shared function.
3. The fit must also hold when the Forces live loop runs after a force layout: nodes it moves are kept on
   screen or the loop's final positions are refitted once, whichever matches the existing contract; cite it.

Paths: `packages/graph-render/**`, `packages/graph-studio/**`, `deploy/nav/**`, `scripts/studio-nav.sh`.

Use the `pw` MCP on the dev server to confirm on `force.drl` and the clustered fixture (screenshot to /out/).

Done when: `scripts/studio.sh check` exits 0; `scripts/studio.sh build && scripts/studio-nav.sh` exits 0
with the new row PASS; `STUDIO_NAV_BREAK=1 scripts/studio-nav.sh` exits non-zero.

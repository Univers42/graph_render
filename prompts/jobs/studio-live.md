# Job studio-live (agent build, Obsidian-style live graph in the studio)

Why: user, 2026-09-30: "the nodes are still not moving and interacting with each other like in
obsidian.. we need all the console interaction progress bar like obsidian". The motor already has the live session
(`crates/graph-core/src/layout/force/session.rs`). The wasm ABI and the SDK `ForceSession` come from the
`force-wasm` job and are on develop (`docs/decisions/force-wasm-abi.md`). The unmerged branch
`studio-force` (head 15ce426) holds most of the studio half: `motor/liveLoop.ts`, `live.ts`,
`liveDrag.ts`, `actions/forces.ts`, `ui/ForcesPanel.tsx`, and the drag in `graph-render/src/drag.ts`,
with tests. Read `git diff develop...origin/studio-force -- packages` first.

First, in this worktree (a branch made from develop): run `git merge --no-commit origin/studio-force`.
Resolve every conflict by editing files. After the merge, never run git commands that change state.

Behaviour to deliver. Obsidian's graph view is the reference:
1. On open, and on every layout change to a force layout, the simulation runs live: the nodes spread
   out and settle on screen, frame by frame. The worker ticks the `ForceSession` and posts positions
   through the zero-copy view, and the renderer redraws each frame.
2. Dragging a node pins it under the pointer and reheats the simulation, so its neighbours and the rest
   of the graph follow and push away. On release the node is unpinned (Obsidian) and the graph settles again.
3. The forces panel (center, repel, link force, link distance) changes the running simulation at once
   through the session setters. Also add an "Animate" button that restarts the settle from random
   positions.
4. Progress: a thin bar at the top of the canvas shows the settle (alpha going down towards its
   minimum), and batch layout runs in the worker show their progress the same way. The bar hides once
   the graph has settled.
5. Console: every forces action is registered once in `src/actions/registry.ts` and callable from the
   console (e.g. `forces.repel 300`, `forces.animate`, `forces.pause`, `forces.resume`), with the result
   and errors printed there. The dock, shortcuts and console all go through `resolve`.
6. Performance: the live loop never blocks the UI thread. Ticks run in the worker, rendering runs on
   requestAnimationFrame, and a slow frame drops ticks and never queues them.

Proof:
- Unit tests: the studio-force tests run against the real SDK session, not a stub. Add tests for the
  progress bar's state machine and for the console commands.
- A browser gate `scripts/studio-live.sh`, modelled on `scripts/studio-nav.sh` and its siblings (read
  their headers): build, open the app with a fixture, and use CDP to drag one node 150 px. It asserts
  that (a) node positions change between frames with no input after load, (b) after the drag, at least
  one neighbour of the dragged node has moved by more than 5 px, and (c) the progress bar is visible
  during the settle and hidden after it. Add a negative control: `STUDIO_LIVE_BREAK=1` disables ticking
  and the gate must exit non-zero.

Checks (paste each last line): `scripts/studio.sh wasm`, `scripts/studio.sh check`, `scripts/studio.sh build`,
`scripts/studio-live.sh`, `STUDIO_LIVE_BREAK=1 scripts/studio-live.sh` (non-zero), `scripts/studio-nav.sh`, and
the quick.rows set.

Paths you may touch: `packages/**`, `app/**`, `scripts/studio-live.sh`, `deploy/nav/**` (if the CDP driver
lives there). Not `crates/` (if the ABI lacks something, stop and name it in `decisions needed`).

Done when: the checks pass, the gate's three assertions pass, the negctl fails, and the return block has
the gate's output lines.

UI evidence (user rule, 2026-10-01): every claim about what the studio shows needs a `pw` MCP snapshot or screenshot, saved under `target/wf/` and named in the return block. The MCP servers (pw, shadcn, context7, deepwiki, ruflo) and the skills are enabled in `opencode.json`; if a `pw.*` tool reports "Unknown tool", the server is still connecting, so wait about 4 s and call `search()` again.

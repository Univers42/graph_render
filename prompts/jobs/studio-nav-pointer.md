# Job studio-nav-pointer (agent build: the camera buttons take a real click)

Why: the HUD's five camera buttons (`⤢ + − 0 ←50→`) never get a real mouse click. The
orchestrator found this on 2026-10-06 with the pw MCP on develop 99a07bd5:
- A Playwright click on "Fit the graph to the view" timed out after 30 s, with "`<canvas
  class="gs-canvas">` intercepts pointer events".
- `elementFromPoint` at the button's centre returns the canvas.
- Every ancestor of the button computes `pointer-events: none`:
  - `.gs-root`, at `packages/graph-studio/src/mount.ts:63`;
  - `.gs-chrome`, at `packages/graph-studio/src/styles/studio.css.ts:27`;
  - `.gs-bottom-left` and `.gs-nav` inherit it.
- Panels opt back in with `.gs-panel { pointer-events: auto }` (`studio.css.ts:47`).
- `.gs-nav` (`:74`) does not opt back in. It is not a panel.

Keyboard shortcuts and `dispatch` still work. That is why no test saw it:
- `tests/ui/hud.test.tsx:71-83` renders the buttons and calls their actions;
- no browser gate clicks them with a pointer.

Facts (develop, 2026-10-06; re-check each on your branch before editing, and stop if one no
longer holds):
- `packages/graph-studio/src/ui/NavBar.tsx:36-50` renders `<div className="gs-nav" role="group"
  aria-label="Camera">` and the five `gs-btn gs-nav-btn` buttons.
- `scripts/studio-interact.sh` is the interaction gate:
  - it drives real CDP input over `app/dist`;
  - it writes `target/studio-interact/<label>/report.json`, label `current` by default;
  - `STUDIO_INTERACT_BREAK=1` passes `--break`.
- `deploy/nav/interactrows.py` holds the gate's rows and is 217 lines. `run_rows` is at
  `:214-217`; `row(name, text, detail, ok)` (`deploy/nav/verdict.py`) builds a row; `report.json` keys it by `"row"`.
- `deploy/nav/drive.py` provides:
  - `Studio.camera()` at `:142`, which reads `view.camera()` (`x`, `y`, `scale`);
  - `Studio.click(at)` at `:175`;
  - `settle` at `:145`.

Steps:
1. **RED.** In `deploy/nav/interactrows.py`, add `row_camera_buttons(studio, broken)`, named
   "camera buttons". For each of the five buttons, located in the shadow root by its
   `aria-label`:
   - `elementFromPoint` at the button's centre is the button or inside it;
   - a real `Studio.click` on "Zoom in ×2" changes `camera()["scale"]` by ×2, within 1e-9
     relative, measured from a settled camera;
   - a real click on "Fit the graph to the view" then moves the camera back to the fit.

   Under `broken`, the row first injects `<style>.gs-nav{pointer-events:none}</style>` into the
   studio's shadow root, so the row must FAIL. Add the row to `run_rows`.

   Run `scripts/studio.sh build && scripts/studio-interact.sh`. The new row must FAIL on today's
   CSS, and every other row must keep its verdict. Record the output.
2. **The fix.** Add `pointer-events: auto;` to `.gs-nav` in `studio.css.ts:74`. Change nothing
   else in the CSS. The bar sits over the canvas and must not pass clicks through.
3. **Green.**
   - Rebuild, then run `scripts/studio-interact.sh`: every row PASS.
   - Run `STUDIO_INTERACT_BREAK=1 scripts/studio-interact.sh`: non-zero, and `report.json`
     shows "camera buttons" as FAIL.
   - Run `scripts/studio.sh check` and the smoke rows.
4. Update the header comment of `scripts/studio-interact.sh`: its "Rows:" line gains "the five
   camera buttons take a real click".

Rules (beyond `scripts/orch/common.md`):
- Node only through `scripts/orch/node-slim.sh` / `scripts/studio.sh`. The browser gates never
  take the host gate lock.
- Paths you may touch:
  - `packages/graph-studio/src/styles/studio.css.ts`, the one line;
  - `deploy/nav/interactrows.py`;
  - the header of `scripts/studio-interact.sh`.
- Nothing under `crates/`, `src/`, `app/`, `packages/graph-render` or the root `package*.json`
  changes.
- `interactrows.py` stays ≤ 300 lines, with each function ≤ 40 lines.

Done when `scripts/orch/gate.sh target/rows-studio-nav-pointer
scripts/orch/rows/studio-nav-pointer.rows` writes a `summary.txt` with every row PASS.

Return:
- the branch tip;
- the files changed, with line counts;
- the RED run from step 1;
- each row's result;
- every deviation.

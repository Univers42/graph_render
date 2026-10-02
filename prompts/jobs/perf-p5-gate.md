# Job perf-p5-gate (agent build; a slice of P5 on branch perf-p5-gate, cut from perf-p5, merged into perf-p5 by the orchestrator)

Why: P5 (`prompts/perf-plan.md` §P5) gave graph-render a WebGL2 layer (`packages/graph-render/src/webgl2/`).
Its gate rows do not exist yet: parity against Canvas2D, the fallback, and a negative control showing that a
broken backend cannot leave a blank canvas unnoticed.

Facts (verified on perf-p5 935a3c3):
- The page URL picks the backend: `?backend=auto|canvas2d|webgl2` (`app/src/main.ts:4`). `auto` uses the GL
  layer from `BULK_THRESHOLD` = 8192 nodes+edges (`webgl2/plan.ts`); `webgl2` forces it at any size.
- `document.querySelector("graph-studio").view.stats()` returns `backend` ("webgl2" when the GL layer drew the
  last frame, else "canvas2d") and `backendFailure` ("" or why the layer was given up). `webgl2/hook.ts` falls
  back to Canvas2D for good when `createBulk()` returns null or throws ("this browser gives no WebGL2 context on
  an OffscreenCanvas") or when the context is lost ("the WebGL2 context was lost").
- The GL layer runs on the page's main thread, on an `OffscreenCanvas` (`webgl2/layer.ts`), and hands each frame
  to the 2D canvas with `transferToImageBitmap` + `drawImage`.
- Headless chromium in `gm-chromium` has WebGL2 only with `--enable-unsafe-swiftshader` (`deploy/perf/run.py`
  `launch_browser(profile, backend)`); `deploy/nav/nav.py` `launch_browser(profile)` passes no such flag.
- Opening an N-node graph with a chosen layout: `deploy/perf/drivers/hook.js` `window.__perf.open(nodes, layout)`.
- The gate to copy the shape from: `scripts/studio-smoke.sh`, `deploy/nav/smoke.py`, `smokecdp.py`, `smokerows.py`.

Do:
1. Read every file named above first.
2. New gate `scripts/studio-backend.sh` with `deploy/nav/backend.py` (and `backendrows.py` to stay under 300
   lines), in `gm-chromium`, chromium launched with `--enable-unsafe-swiftshader`. Give `nav.launch_browser`
   an optional extra-arguments parameter rather than copying it. Rows:
   - `backend-parity-2k`: one 2000-node synthetic graph with a fixed seed and a deterministic layout, drawn at
     `?backend=canvas2d` and at `?backend=webgl2`, both settled, same camera and viewport. Compare the two canvas
     screenshots: the fraction of pixels whose largest channel difference exceeds a threshold. Measure first,
     then set the ceiling at the next round value above the measurement, with a `Caveat:` line saying what the
     comparison cannot see. Both screenshots must also be non-blank: at least 1% of pixels differ from the
     background colour.
   - `backend-auto-large`: `?backend=auto` with a 20000-node graph; `stats().backend == "webgl2"`.
   - `backend-fallback`: `?backend=webgl2` with WebGL2 removed before the page's own scripts
     (`Page.addScriptToEvaluateOnNewDocument`: `OffscreenCanvas.prototype.getContext` returns null for
     "webgl2"). Expect `backend == "canvas2d"`, a non-empty `backendFailure`, a non-blank canvas, no page error.
   - `backend-context-lost`: `?backend=webgl2`, a wrapper records the layer's WebGL2 context, the gate calls
     `getExtension("WEBGL_lose_context").loseContext()` and moves the camera one step. Expect
     `backend == "canvas2d"`, `backendFailure` naming the lost context, a non-blank canvas.
3. Negative control `STUDIO_BACKEND_BREAK=1`: a backend that silently draws nothing, made by stubbing
   `WebGL2RenderingContext.prototype.drawArrays`, `drawElements`, `drawArraysInstanced` and
   `drawElementsInstanced` to no-ops before the page loads. The parity row must go red and the gate exit non-zero.
4. Save the screenshots and `report.json` under `target/studio-backend/`.
5. Add one line for the gate to CLAUDE.md's studio command block, after `studio-smoke.sh`.

Paths: `scripts/studio-backend.sh`, `deploy/nav/**`, CLAUDE.md (that one line). Nothing under `packages/` or
`crates/`: if a row cannot be written without a package change, stop and say which.

Done when: `scripts/studio.sh check` exits 0; `scripts/studio-backend.sh` exits 0 with every row PASS;
`STUDIO_BACKEND_BREAK=1 scripts/studio-backend.sh` exits non-zero; `shellcheck scripts/studio-backend.sh` is
clean. The return block pastes each command's real exit code and the gate's table.

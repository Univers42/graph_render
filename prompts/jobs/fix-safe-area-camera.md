# Job fix-safe-area-camera (agent build: a chrome resize never moves the camera)

Why: on develop, selecting many nodes grows the Inspector, the safe area changes and the camera
re-fits under the pointer. `int-box` is red and `int-node-drag` is flaky (10.33 px in 1 of 5 runs).

Facts (measured 2026-10-03 by fix-node-drag, `$GM_SCRATCH/orch/logs/job-fix-node-drag.out`; confirm first):
- A large selection grows `.gs-left` from 239 to 455 px. `watchSafeArea` re-declares the safe
  area, and `setSafeArea` (`packages/graph-render/src/canvas2d/controller.ts:131`) re-fits the
  camera, which moves it about 100 px mid-row.
- Three `setSafeArea` rules were measured; none passes all three rows:

  | rule | `int-box` | `int-node-drag` | `backend-parity-2k` |
  |---|---|---|---|
  | always fit (develop) | FAIL | PASS 4/5 | PASS |
  | fit only if the drawing is cropped | PASS | FAIL 255.87 px | FAIL |
  | fit at the next idle frame | FAIL | FAIL | PASS |

Decision (orchestrator, 2026-10-03): a safe-area change never moves the camera. It refreshes the
pan and zoom limits only. The camera is fitted where a layout arrives (`setFrame`), and
`backend-parity-2k` takes its camera from that fit. If the parity probe sets its own camera through
`setSafeArea`, move that to the `setFrame` fit; the parity tolerance does not move.

Do, in order:
1. Reproduce: `scripts/studio.sh wasm && scripts/studio.sh build && scripts/studio-interact.sh`
   (expect `int-box` FAIL), and `scripts/studio-backend.sh`.
2. Apply the decision at `setSafeArea` and wherever the first fit must now happen.
3. One unit test in `packages/graph-render` that fails before the fix: a safe-area change after a
   fit leaves the camera's pan and zoom unchanged.

Paths: `packages/graph-render/src/**`, `packages/graph-studio/src/**` and their tests;
`deploy/nav/*.py` only to read.

Out of bounds: `crates/`; any probe tolerance.

No new dependency. House limits: 40 lines per function, 300 lines per file, 4 parameters.

Done when:
- `scripts/orch/rows/ux-forces.rows` is green, each negative control non-zero, and
  `scripts/studio-interact.sh` passes 5 runs in a row (paste the five `int-node-drag` distances).
- A pw MCP pass on the built studio: box-select many nodes, and save before and after screenshots
  under `docs/measurements/fix-safe-area-camera/`. Read the store error, the console exceptions and
  the overlay text; any one non-empty is a FAIL.

Resolution (2026-10-03, orchestrator): not run. Develop dbab64cb already carries the `gestured` rule
(`controller.ts` `setSafeArea` re-fits only before the first user gesture) and
`tests/safe-area-camera.test.ts`. Measured on that tree (`$GM_SCRATCH/orch/logs/safe-area-repro.out`):
`studio-interact.sh` 3/3 green, `int-box` 364 inside / 364 selected, `int-node-drag` 0.00 px each run;
`studio-backend.sh` green, `backend-parity-2k` 0.5835 %.

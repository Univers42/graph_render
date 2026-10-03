/**
 * What a host's safe-area change does to the camera, read without a browser.
 *
 * The regression: the chrome resizes for reasons that have nothing to do with the drawing — a
 * selection filling the Inspector is the one that mattered — and every one of those resized the
 * camera under whatever the user was pointing at. The box-select row measured its box against
 * the camera it had before the refit and disagreed with itself.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Controller } from "../src/canvas2d/controller.ts";
import { setSafeArea } from "../src/canvas2d/controller.ts";

/** The smallest controller `setSafeArea` reads: the state it touches and nothing else. */
function controller(over: { readonly camera?: Controller["state"]["camera"] } = {}): Controller {
  const camera = over.camera ?? { scale: 0.4, x: 700, y: 325 };
  return {
    fitted: true,
    state: {
      camera,
      safe: null,
      viewport: { width: 1400, height: 900 },
      scene: { bounds: { minX: -1200, maxX: 1200, minY: -900, maxY: 900 } },
    },
    local: { bounds: null },
  } as unknown as Controller;
}

test("a panel that grew and still shows the whole drawing does not move the camera", () => {
  // The camera shows world [-1200,1200] as screen [220,1180] x [325-360, 325+360]: inside a
  // 1400x900 canvas with a 260px left column, before and after the Inspector grows.
  const made = controller();
  const before = { ...made.state.camera };
  setSafeArea(made, { x: 272, y: 0, width: 1128, height: 900 });
  assert.deepEqual(made.state.camera, before, "the drawing is still whole in the free box, so it stays put");
  assert.deepEqual(made.state.safe, { x: 272, y: 0, width: 1128, height: 900 }, "and the area is still recorded");
});

test("a panel that grew over the drawing does re-fit, which is what the safe area is for", () => {
  const made = controller();
  // A free box the drawing no longer fits: the fit has to move or the nodes are under the panel.
  setSafeArea(made, { x: 272, y: 0, width: 200, height: 900 });
  assert.notDeepEqual(made.state.camera, { scale: 0.4, x: 700, y: 325 }, "the camera was re-fitted into the smaller box");
});

test("an unfitted camera is never re-fitted by a safe-area change", () => {
  const made = controller({ camera: { scale: 1.7, x: -40, y: 12 } });
  made.fitted = false;
  const before = { ...made.state.camera };
  setSafeArea(made, { x: 272, y: 0, width: 200, height: 900 });
  assert.deepEqual(made.state.camera, before, "the user chose this camera; a panel moving is not a reason to take it");
});
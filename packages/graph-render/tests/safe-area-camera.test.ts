/**
 * What a host's safe-area change does to the camera, read without a browser.
 *
 * The rule: a safe-area change moves the camera only while the camera is still the view's own
 * automatic fit, that is before the first gesture on the canvas. After one, the camera is the
 * user's and the area only bounds how far the drawing may be panned.
 *
 * Why it is written down: the chrome resizes for reasons that have nothing to do with the
 * drawing — a selection filling the Inspector is the one that mattered — and re-fitting on each of
 * those took whatever the user was pointing at out from under the pointer. The box-select row then
 * measured a screen-space box against a camera that had moved, and disagreed with itself.
 *
 * The controller is driven directly: `createView` needs a ResizeObserver and a
 * requestAnimationFrame that node does not have (`live-positions.test.ts` says the same).
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Controller } from "../src/canvas2d/controller.ts";
import { newState, setSafeArea, showFrame } from "../src/canvas2d/controller.ts";
import { taken } from "../src/gestured.ts";
import type { LoopState } from "../src/canvas2d/loop.ts";
import { lineFrame } from "./support.ts";

function isCanvas(value: unknown): value is HTMLCanvasElement {
  return typeof value === "object" && value !== null && "getContext" in value;
}

/** A state with a canvas 1400x900 and two nodes spread over the middle of the world. */
function wide(): LoopState {
  const canvas: unknown = { getContext: () => ({}) };
  if (!isCanvas(canvas)) throw new Error("the stand-in canvas lost its getContext");
  const state = newState(canvas, { theme: undefined, policy: undefined, onFrame: () => {} });
  state.viewport = { width: 1400, height: 900 };
  showFrame(state, lineFrame({ x: [-300, 300], y: [-200, 200], edges: [[0, 1]] }), false);
  state.destroyed = true; // invalidate() then schedules nothing: node has no requestAnimationFrame
  return state;
}

/** `setSafeArea` reads four fields of the controller; the rest of it is a browser's job. */
function isController(value: unknown): value is Controller {
  return typeof value === "object" && value !== null && "fitted" in value && "state" in value;
}

function controller(camera = { scale: 0.4, x: 700, y: 450 }): Controller {
  const state = wide();
  state.camera = { ...camera };
  const made = {
    state, fitted: true, gestured: false, local: { bounds: null }, canvas: {},
    notify: { camera: () => {}, hover: () => {}, select: () => {}, selection: () => {}, context: () => {} },
  };
  if (!isController(made)) throw new Error("the stand-in controller lost what setSafeArea reads");
  return made;
}

/** The studio's own chrome: a 272px left column, which is the box the drawing must sit inside. */
const CHROME = { x: 272, y: 0, width: 1128, height: 900 };

test("before the first gesture a safe-area change re-fits, because the camera is still the view's", () => {
  const made = controller();
  const before = { ...made.state.camera };
  setSafeArea(made, CHROME);
  assert.notDeepEqual(made.state.camera, before, "the drawing was fitted into the free box, not the whole canvas");
  assert.deepEqual(made.state.safe, CHROME, "and the area is recorded");
});

/** One gesture on the canvas: the click `view.ts` wraps, which is what ends the automatic fit. */
function gesture(controller: Controller): void {
  taken(controller, () => undefined)();
}

test("after the first gesture a safe-area change leaves the camera exactly where it was", () => {
  const made = controller();
  gesture(made);
  const before = { ...made.state.camera };
  // The studio's own chrome resizing: the Inspector filling with a selection, then the dock moving.
  setSafeArea(made, { ...CHROME, width: 1120 });
  assert.deepEqual(made.state.camera, before, "the drawing did not move under the pointer");
  setSafeArea(made, null);
  assert.deepEqual(made.state.camera, before, "and losing the panels does not move it either");
  assert.deepEqual(made.state.safe, null, "though the area is still recorded");
});

test("after a gesture the area still bounds how far the drawing may be panned", () => {
  const made = controller();
  gesture(made);
  setSafeArea(made, CHROME);
  assert.ok(made.state.limits.max > 0, "the limits were recomputed from the new free box");
  assert.deepEqual(made.state.safe, CHROME);
});

test("a stale or inverted box is refused, and the canvas is the answer", () => {
  const made = controller();
  for (const bad of [{ x: 0, y: 0, width: 0, height: 0 }, { x: Number.NaN, y: 0, width: 10, height: 10 }]) {
    setSafeArea(made, bad);
    assert.equal(made.state.safe, null, "the whole canvas stands in");
  }
});
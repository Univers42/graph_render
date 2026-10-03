/**
 * What a host's safe-area change does to the camera, read without a browser.
 *
 * The regression: the chrome resizes for reasons that have nothing to do with the drawing — a
 * selection filling the Inspector is the one that mattered — and every one of those resized the
 * camera under whatever the user was pointing at. The box-select row measured its box against
 * the camera it had before the refit and disagreed with itself.
 *
 * The controller is driven directly: `createView` needs a ResizeObserver and a
 * requestAnimationFrame that node does not have (`live-positions.test.ts` says the same).
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Controller } from "../src/canvas2d/controller.ts";
import { newState, setSafeArea, showFrame } from "../src/canvas2d/controller.ts";
import type { LoopState } from "../src/canvas2d/loop.ts";
import { lineFrame } from "./support.ts";

function isCanvas(value: unknown): value is HTMLCanvasElement {
  return typeof value === "object" && value !== null && "getContext" in value;
}

/** A state with a canvas 1400x900 and four nodes spread over the middle of the world. */
function wide(): LoopState {
  const canvas: unknown = { getContext: () => ({}) };
  if (!isCanvas(canvas)) throw new Error("the stand-in canvas lost its getContext");
  const state = newState(canvas, { theme: undefined, policy: undefined, onFrame: () => {} });
  state.viewport = { width: 1400, height: 900 };
  showFrame(state, lineFrame({ x: [-300, 300], y: [-200, 200], edges: [[0, 1]] }), false);
  state.destroyed = true; // invalidate() then schedules nothing: node has no requestAnimationFrame
  return state;
}

function fitted(over: Partial<Controller> = {}): Controller {
  const state = wide();
  const made = {
    state, fitted: true, local: { bounds: null }, canvas: {},
    notify: { camera: () => {}, hover: () => {}, select: () => {}, selection: () => {}, context: () => {} },
  } as unknown as Controller;
  return Object.assign(made, over);
}

/** Fits the drawing to the whole canvas, which is what a fresh view does with `setSafeArea(null)`. */
function fitCameraFor(made: Controller): void {
  // `setSafeArea(null)` on an already-null area is a no-op, so nudge it through a change and back.
  setSafeArea(made, { x: 0, y: 0, width: made.state.viewport.width, height: made.state.viewport.height });
  setSafeArea(made, null);
}

/** Where the drawing sits on screen, as `[left, top, right, bottom]`. */
function extent(state: LoopState): number[] {
  const { scale } = state.camera;
  const b = state.scene.bounds;
  if (b === null) throw new Error("the frame has no bounds");
  return [b.minX * scale + state.camera.x, b.minY * scale + state.camera.y,
    b.maxX * scale + state.camera.x, b.maxY * scale + state.camera.y].map((v) => Math.round(v));
}

test("a safe area that still shows the whole drawing does not move the camera", () => {
  const made = fitted();
  // The camera a fresh view starts from: the whole drawing fitted to the whole canvas.
  fitCameraFor(made);
  const before = { ...made.state.camera };
  const was = extent(made.state);
  // The studio's own chrome: a 272px left column with the Inspector open, which is exactly the
  // change that used to move the camera under a box selection. The drawing is drawn well inside
  // it, so nothing about it needs to move.
  setSafeArea(made, { x: 272, y: 0, width: 1128, height: 900 });
  assert.deepEqual(made.state.camera, before, "the drawing is still whole in the free box, so it stays put");
  assert.deepEqual(extent(made.state), was, "so it is drawn in exactly the same place");
  assert.deepEqual(made.state.safe, { x: 272, y: 0, width: 1128, height: 900 }, "and the area is still recorded");
});

test("a safe area too small for the drawing does re-fit, which is what it is for", () => {
  const made = fitted();
  fitCameraFor(made);
  const before = { ...made.state.camera };
  setSafeArea(made, { x: 272, y: 0, width: 120, height: 900 });
  assert.notDeepEqual(made.state.camera, before, "the camera was re-fitted into the narrower box");
});

test("an unfitted camera is never re-fitted by a safe-area change", () => {
  const made = fitted({ fitted: false });
  made.state.camera = { scale: 1.7, x: -40, y: 12 };
  const before = { ...made.state.camera };
  setSafeArea(made, { x: 272, y: 0, width: 120, height: 900 });
  assert.deepEqual(made.state.camera, before, "the user chose this camera; a panel moving is no reason to take it");
});
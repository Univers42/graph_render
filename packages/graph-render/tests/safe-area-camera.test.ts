/**
 * What a host's safe-area change does to the camera, read without a browser.
 *
 * Recorded here because it is a known open defect, not because the code does this: the chrome
 * resizes for reasons that have nothing to do with the drawing — a selection filling the
 * Inspector is the one that mattered — and each of those re-fits the camera, so a probe that
 * measured a screen-space box before the box changed measures a different box afterwards. The
 * `int-box` row is the one that shows it, and the fix is not in this file's subject: see the
 * job's return block.
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

/** `setSafeArea` reads three fields of the controller; the rest of it is a browser's job. */
function isController(value: unknown): value is Controller {
  return typeof value === "object" && value !== null && "fitted" in value && "state" in value;
}

function fitted(camera = { scale: 0.4, x: 700, y: 450 }): Controller {
  const state = wide();
  state.camera = { ...camera };
  const made = {
    state, fitted: true, local: { bounds: null }, canvas: {},
    notify: { camera: () => {}, hover: () => {}, select: () => {}, selection: () => {}, context: () => {} },
  };
  if (!isController(made)) throw new Error("the stand-in controller lost what setSafeArea reads");
  return made;
}

test("a safe-area change is recorded and the limits recomputed from it", () => {
  const made = fitted();
  setSafeArea(made, { x: 272, y: 0, width: 1128, height: 900 });
  assert.deepEqual(made.state.safe, { x: 272, y: 0, width: 1128, height: 900 });
  assert.ok(made.state.limits.max > 0, "and the limits were recomputed from it");
});

test("a stale or inverted box is refused, and the canvas is the answer", () => {
  const made = fitted();
  for (const bad of [{ x: 0, y: 0, width: 0, height: 0 }, { x: Number.NaN, y: 0, width: 10, height: 10 }]) {
    setSafeArea(made, bad);
    assert.equal(made.state.safe, null, "the whole canvas stands in");
  }
});
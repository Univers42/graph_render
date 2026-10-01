/**
 * A live simulation's positions: installed in place, drawn, picked, and reported. The
 * controller is driven directly — `createView` needs an OffscreenCanvas, a ResizeObserver
 * and a requestAnimationFrame that node does not have, and `drag.test.ts` stands in a canvas
 * the controller only reads `getContext` from for the same reason.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type LoopState } from "../src/canvas2d/loop.ts";
import { newState, pickAt, setPositions, showFrame } from "../src/canvas2d/controller.ts";
import { lineFrame } from "./support.ts";

// The controller reads only getContext from the canvas; the guard stands in for a DOM that node lacks.
function isCanvas(value: unknown): value is HTMLCanvasElement {
  return typeof value === "object" && value !== null && "getContext" in value;
}

/** Three nodes on a line under the world origin: canvas coordinates are world coordinates. */
function threeNodes(): LoopState {
  const canvas: unknown = { getContext: () => ({}) };
  if (!isCanvas(canvas)) throw new Error("the stand-in canvas lost its getContext");
  const setup = { theme: undefined, policy: undefined, onFrame: () => {} };
  const state = newState(canvas, setup);
  showFrame(state, lineFrame({ x: [0, 100, 200], y: [0, 0, 0], edges: [[0, 1]] }), false);
  state.camera = { x: 0, y: 0, scale: 1 };
  state.destroyed = true; // invalidate() then schedules nothing: node has no requestAnimationFrame
  return state;
}

/** What `view.position` returns: the columns the view draws from. */
function position(state: LoopState, node: number) {
  return { x: state.x[node] ?? 0, y: state.y[node] ?? 0 };
}

test("a live pair of the right length is drawn where it says", () => {
  const state = threeNodes();
  setPositions(state, Float64Array.from([0, 150, 200]), Float64Array.from([0, 300, 0]));
  assert.deepEqual(position(state, 1), { x: 150, y: 300 });
  assert.deepEqual(position(state, 0), { x: 0, y: 0 }, "the nodes it did not move stay");
});

test("a pair of another graph's length is ignored", () => {
  const state = threeNodes();
  setPositions(state, Float64Array.from([9, 9, 9, 9, 9]), Float64Array.from([9, 9, 9, 9, 9]));
  assert.deepEqual(position(state, 0), { x: 0, y: 0 });
  assert.equal(pickAt(state, { x: 100, y: 0 }), 1, "the drawing is the one that is on screen");
});

test("the pick grid followed the live positions, and picking did not stop", () => {
  const state = threeNodes();
  setPositions(state, Float64Array.from([0, 150, 200]), Float64Array.from([0, 300, 0]));
  assert.equal(pickAt(state, { x: 150, y: 300 }), 1);
  assert.equal(pickAt(state, { x: 100, y: 0 }), -1, "the node left its old place");
  assert.equal(state.transitionStart, -1, "a live settle is not a transition");
});

test("the frame reports the positions on screen, from the drawn columns", () => {
  const state = threeNodes();
  setPositions(state, Float64Array.from([0, 150, 200]), Float64Array.from([0, 300, 0]));
  assert.equal(state.scene.frame.x, state.x, "the frame and the drawing are one array");
  assert.deepEqual([state.scene.frame.x[1], state.scene.frame.y[1]], [150, 300]);
});
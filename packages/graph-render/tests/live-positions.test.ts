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

/**
 * A batch carrying one non-finite coordinate is refused whole, the way the snapshot reader
 * refuses one (`snapshot/decode.ts:116`): a NaN would become a drawn sprite, a pick-grid entry
 * and a NaN camera, and half a batch of them is not a drawing either. The control is the row
 * above it — a finite batch of the same shape IS installed — so this row can only fail for the
 * finiteness, not for the length or the plumbing.
 */
test("a batch with one non-finite coordinate is refused whole", () => {
  for (const bad of [Number.NaN, Infinity, -Infinity]) {
    const state = threeNodes();
    setPositions(state, Float64Array.from([0, 150, 200]), Float64Array.from([0, 300, bad]));
    assert.deepEqual(position(state, 1), { x: 100, y: 0 }, `a ${bad} y on node 2 changes nothing`);
    assert.deepEqual(position(state, 2), { x: 200, y: 0 }, "not even the nodes before it move");
    assert.equal(pickAt(state, { x: 100, y: 0 }), 1, "the drawing is still the one on screen");
    assert.ok(Number.isFinite(state.camera.scale), "and the camera is still a camera");
    assert.ok(Number.isFinite(state.limits.min) && Number.isFinite(state.limits.max), "as are the limits");
  }
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
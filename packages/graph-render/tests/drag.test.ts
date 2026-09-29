/** A dragged node: the scene is rebuilt around its new position, and only then. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { EMPTY_FRAME, pickIn, sceneOf } from "../src/scene.ts";
import { movedScene } from "../src/drag.ts";
import { plainStyle } from "../src/style.ts";
import { newState, showFrame } from "../src/canvas2d/controller.ts";
import { pressAt } from "../src/canvas2d/choose.ts";
import { lineFrame } from "./support.ts";
import { newLocalLayer } from "../src/local.ts";

function threeNodes() {
  const x = Float32Array.from([0, 100, 200]);
  const y = Float32Array.from([0, 0, 0]);
  const frame = {
    ...EMPTY_FRAME, nodeCount: 3, edgeCount: 1, x, y,
    source: Uint32Array.from([0]), target: Uint32Array.from([1]),
    bounds: { minX: 0, minY: 0, maxX: 200, maxY: 0 },
  };
  return sceneOf(frame, plainStyle(3), null);
}

const HERE = { tolerance: 1, floor: 4 };

test("after the move the node is picked at its new place and not at the old one", () => {
  const scene = threeNodes();
  const x = Float32Array.from([0, 100, 200]);
  const y = Float32Array.from([0, 0, 0]);
  x[1] = 150;
  y[1] = 300;
  const moved = movedScene(scene, { x, y });
  assert.equal(pickIn(moved, { x: 150, y: 300, ...HERE }), 1);
  assert.equal(pickIn(moved, { x: 100, y: 0, ...HERE }), -1);
  assert.equal(pickIn(scene, { x: 100, y: 0, ...HERE }), 1, "the old scene is untouched");
});

test("the moved scene keeps the adjacency and the edge ends, and reads the new columns", () => {
  const scene = threeNodes();
  const x = Float32Array.from([5, 100, 200]);
  const moved = movedScene(scene, { x, y: scene.frame.y });
  assert.equal(moved.adjacency, scene.adjacency);
  assert.equal(moved.frame.x, x);
  assert.equal(moved.frame.source, scene.frame.source);
});

// The controller reads only getContext from the canvas; the guard stands in for a DOM that node lacks.
function isCanvas(value: unknown): value is HTMLCanvasElement {
  return typeof value === "object" && value !== null && "getContext" in value;
}

function dragOneNode() {
  const canvas: unknown = { getContext: () => ({}) };
  if (!isCanvas(canvas)) throw new Error("the stand-in canvas lost its getContext");
  const setup = { theme: undefined, policy: undefined, onFrame: () => {} };
  const state = newState(canvas, setup);
  const frame = lineFrame({ x: [0, 100, 200], y: [0, 0, 0], edges: [[0, 1]] });
  showFrame(state, frame, false);
  state.camera = { x: 0, y: 0, scale: 1 };
  state.destroyed = true; // invalidate() then schedules nothing: node has no requestAnimationFrame
  const controller = { canvas, state, notify: { hover() {}, select() {}, selection() {}, context() {}, camera() {} }, fitted: true, local: newLocalLayer() };
  return { state, frame, controller };
}

test("a re-layout clears the drag override: the node is back where the new frame puts it", () => {
  const { state, frame, controller } = dragOneNode();
  const gesture = pressAt(controller, { x: 100, y: 0 }, false);
  assert.notEqual(gesture, null);
  gesture?.move({ x: 250, y: 300 });
  assert.deepEqual([state.x[1], state.y[1]], [250, 300], "the drag put the node under the pointer");
  showFrame(state, lineFrame({ x: [0, 100, 200], y: [0, 0, 0], edges: [[0, 1]] }), false);
  assert.deepEqual([state.x[1], state.y[1]], [100, 0], "the new frame's position wins");
  assert.equal(frame.x[1], 100, "the dragged copy never wrote into the frame");
});

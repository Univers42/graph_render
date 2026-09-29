/** A dragged node: the scene is rebuilt around its new position, and only then. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { EMPTY_FRAME, pickIn, sceneOf } from "../src/scene.ts";
import { movedScene } from "../src/drag.ts";
import { plainStyle } from "../src/style.ts";

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

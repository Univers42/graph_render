/** Which nodes a click, a shift-click and a box leave selected. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { addTo, boxOf, nodesInBox } from "../src/selection.ts";

const X = Float32Array.from([0, 10, 20, 30]);
const Y = Float32Array.from([0, 10, 20, 30]);

test("a box is the same whichever corner the drag began at", () => {
  assert.deepEqual(boxOf({ x: 5, y: 40 }, { x: -5, y: 2 }), { minX: -5, minY: 2, maxX: 5, maxY: 40 });
});

test("the nodes in a box are the ones whose centres are on or inside it, in index order", () => {
  assert.deepEqual(nodesInBox({ x: X, y: Y }, null, { minX: 10, minY: 10, maxX: 30, maxY: 30 }), [1, 2, 3]);
  assert.deepEqual(nodesInBox({ x: X, y: Y }, null, { minX: 11, minY: 0, maxX: 19, maxY: 40 }), []);
});

test("a hidden node is never in a box", () => {
  const hidden = Uint8Array.from([0, 1, 0, 0]);
  assert.deepEqual(nodesInBox({ x: X, y: Y }, hidden, { minX: 0, minY: 0, maxX: 20, maxY: 20 }), [0, 2]);
});

test("adding a node keeps the order and never repeats one", () => {
  assert.deepEqual(addTo([3, 1], 2), [3, 1, 2]);
  assert.deepEqual(addTo([3, 1], 3), [3, 1]);
  assert.deepEqual(addTo([], 0), [0]);
  assert.deepEqual(addTo([4], -1), [4], "no node is not a node");
});

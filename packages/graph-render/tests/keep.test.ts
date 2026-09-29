/** View-only marks: pinning toggles, hiding copies the style and takes the node out of picking. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { toggled, withHidden } from "../src/canvas2d/keep.ts";
import { EMPTY_FRAME, pickIn, sceneOf } from "../src/scene.ts";
import { plainStyle } from "../src/style.ts";

function scene() {
  const frame = {
    ...EMPTY_FRAME, nodeCount: 3, x: Float32Array.from([0, 100, 200]), y: new Float32Array(3),
    bounds: { minX: 0, minY: 0, maxX: 200, maxY: 0 },
  };
  return sceneOf(frame, plainStyle(3), null);
}

test("toggling pins a node, then unpins it; a node outside the frame changes nothing", () => {
  assert.deepEqual(toggled([], 1, 3), [1]);
  assert.deepEqual(toggled([1, 2], 1, 3), [2]);
  assert.deepEqual(toggled([1], 3, 3), [1]);
  assert.deepEqual(toggled([1], -1, 3), [1]);
});

test("a hidden node is not picked, the others are, and the old scene keeps its style", () => {
  const before = scene();
  const after = withHidden(before, [1, 9]);
  assert.equal(pickIn(after, { x: 100, y: 0, tolerance: 1, floor: 4 }), -1);
  assert.equal(pickIn(after, { x: 200, y: 0, tolerance: 1, floor: 4 }), 2);
  assert.equal(before.style.hidden, null);
  assert.deepEqual(Array.from(after.style.hidden ?? []), [0, 1, 0]);
  assert.equal(after.grid, before.grid);
});

test("hiding twice accumulates", () => {
  const twice = withHidden(withHidden(scene(), [0]), [2]);
  assert.deepEqual(Array.from(twice.style.hidden ?? []), [1, 0, 1]);
});

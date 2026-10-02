/**
 * The depth sort: the order the 3D painter walks. Two claims matter and both have a
 * negative control here — the furthest node is painted first, and two nodes at the same
 * depth keep dense index order so a repaint of the same drawing is the same drawing.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { NEAR } from "../src/three/orbit.ts";
import { depthOrder } from "../src/three/sort.ts";

function order(depths: readonly number[]): readonly number[] {
  return [...depthOrder(Float64Array.from(depths), new Uint32Array(depths.length))];
}

test("the furthest node is painted first, so the nearest is drawn over it", () => {
  assert.deepEqual(order([10, 300, 100]), [1, 2, 0]);
});

test("two nodes at one depth keep dense index order, whichever way they arrived", () => {
  assert.deepEqual(order([100, 100, 100]), [0, 1, 2]);
  // The same depths in another order give another order, and that is the caller's choice:
  // what must not happen is the same depths giving two different orders.
  assert.deepEqual(order([100, 100]), [0, 1]);
  assert.deepEqual(order([50, 100, 50, 100]), [1, 3, 0, 2]);
});

test("the order is the reverse of a node-per-bucket fit of the same depths", () => {
  const depths = [12, 900, 340, 7, 55];
  const painted = order(depths);
  for (let at = 1; at < painted.length; at += 1) {
    const was = depths[painted[at - 1] ?? 0] ?? 0;
    const now = depths[painted[at] ?? 0] ?? 0;
    assert.ok(was >= now, `depth ${was} is painted before ${now}`);
  }
});

test("a node at or behind the eye is not in the order at all", () => {
  assert.deepEqual(order([100, -50, 300, 0, NEAR]), [2, 0], "only the two in front are painted");
  assert.deepEqual(order([-1, -2, -3]), [], "a drawing wholly behind the eye paints nothing");
});

test("no drawing is an empty order, not a crash", () => {
  assert.deepEqual(order([]), []);
  assert.deepEqual(order([0, 0, 0]), []);
});

test("an order too short for the depths is a RangeError, not a silent truncation", () => {
  // A truncation here would paint the drawing without some of its nodes and the row would
  // still pass, so the refusal is the whole point of the check.
  assert.throws(() => depthOrder(Float64Array.of(1, 2, 3), new Uint32Array(2)), RangeError);
});

test("the order holds every node in front, once each", () => {
  const depths = Array.from({ length: 500 }, (_, at) => 1 + ((at * 37) % 400));
  const painted = order(depths);
  const seen = new Set(painted);
  assert.equal(painted.length, 500);
  assert.equal(seen.size, 500, "no node is painted twice");
  for (let node = 0; node < 500; node += 1) assert.ok(seen.has(node), `node ${node} is in the order`);
});

test("the order is stable across repeated runs of the same depths", () => {
  const depths = Float64Array.from({ length: 64 }, (_, at) => 10 + (at % 7));
  const first = [...depthOrder(depths, new Uint32Array(64))];
  for (let run = 0; run < 4; run += 1) {
    assert.deepEqual([...depthOrder(depths, new Uint32Array(64))], first, `run ${run}`);
  }
});

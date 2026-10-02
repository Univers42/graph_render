/** The pure helpers that keep a large GPU frame fast: the spread edge order, the moving budget, the quad culling. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { MOVING_BUDGET } from "../src/canvas2d/edges.ts";
import { FAST_MS, SLOW_MS, gathered, largestHalf, nextBudget, onScreen, spreadOrder, spreadPairs, spreadShown } from "../src/webgl2/plan.ts";

test("the spread order is a permutation whose every prefix covers the range evenly", () => {
  assert.deepEqual([...spreadOrder(8)], [0, 4, 2, 6, 1, 5, 3, 7]);
  assert.deepEqual([...spreadOrder(5)], [0, 4, 2, 1, 3]);
  assert.deepEqual([...spreadOrder(0)], []);
  const order = spreadOrder(1000);
  assert.deepEqual([...order].sort((a, b) => a - b), Array.from({ length: 1000 }, (_, at) => at));
  const quarter = [...order.slice(0, 250)];
  for (let tenth = 0; tenth < 10; tenth += 1) {
    const inside = quarter.filter((edge) => edge >= tenth * 100 && edge < (tenth + 1) * 100).length;
    assert.ok(inside >= 20 && inside <= 30, `tenth ${tenth} holds ${inside} of the first 250`);
  }
});

test("spread pairs move each edge's two ends together", () => {
  const pairs = Uint32Array.from([10, 11, 20, 21, 30, 31]);
  assert.deepEqual([...spreadPairs(pairs)], [10, 11, 30, 31, 20, 21]);
});

test("the spread node order skips hidden nodes and keeps the spread of the rest", () => {
  const halves = Float32Array.from([1, 1, -1, -1, 2, 2, 3, 3, 4, 4]);
  assert.deepEqual([...spreadShown(halves)], [0, 4, 2, 3]);
  assert.deepEqual([...spreadShown(new Float32Array(0))], []);
});

test("the moving budget halves when slow, doubles when fast, and stays inside its floor and the whole set", () => {
  assert.equal(nextBudget(64000, SLOW_MS + 1, 1e6), 32000);
  assert.equal(nextBudget(64000, FAST_MS - 1, 1e6), 128000);
  assert.equal(nextBudget(64000, (SLOW_MS + FAST_MS) / 2, 1e6), 64000);
  assert.equal(nextBudget(MOVING_BUDGET, SLOW_MS + 1, 1e6), MOVING_BUDGET);
  assert.equal(nextBudget(800000, FAST_MS - 1, 1e6), 1e6);
  assert.equal(nextBudget(MOVING_BUDGET, SLOW_MS + 1, 100), 100);
});

test("the largest half is the widest side of any shown node, and 0 when none is shown", () => {
  assert.equal(largestHalf(Float32Array.from([1, 2, 5, 3, -1, -1])), 5);
  assert.equal(largestHalf(Float32Array.from([-1, -1])), 0);
});

test("on screen keeps the shown nodes whose padded box meets the viewport, in index order", () => {
  const x = Float32Array.from([0, 50, 100, 200, 50]);
  const y = Float32Array.from([0, 50, 50, 50, 50]);
  const halves = Float32Array.from([1, 1, 1, 1, 1, 1, 4, 4, -1, -1]);
  const view = { x, y, halves, camera: { x: 0, y: 0, scale: 2 }, viewport: { width: 200, height: 200 }, pad: 1 };
  // node 0 straddles the corner, 1 is inside, 2 is at x=200 (on the edge), 3 is at x=400 with reach 9, 4 is hidden
  assert.deepEqual([...onScreen(view)], [0, 1, 2]);
  assert.deepEqual([...onScreen({ ...view, camera: { x: -200, y: 0, scale: 2 } })], [2, 3]);
});

test("gathered copies each listed node's lanes into a column of the same type", () => {
  const halves = Float32Array.from([1, 2, 3, 4, 5, 6]);
  assert.deepEqual([...gathered(halves, Uint32Array.from([2, 0]), 2)], [5, 6, 1, 2]);
  const slots = gathered(Uint16Array.from([7, 8, 9]), Uint32Array.from([1]), 1);
  assert.ok(slots instanceof Uint16Array);
  assert.deepEqual([...slots], [8]);
});

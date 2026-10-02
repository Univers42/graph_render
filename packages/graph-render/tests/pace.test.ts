// The moving edge budget of the 2D canvas: halved by a late frame, doubled back by a run of on-time ones.
import assert from "node:assert/strict";
import { test } from "node:test";

import { MOVING_BUDGET } from "../src/canvas2d/edges.ts";
import { newPace, paced, worthPacing } from "../src/canvas2d/pace.ts";
import { MOVING_FLOOR } from "../src/webgl2/plan.ts";

test("a late frame halves the budget, never below the floor", () => {
  const pace = newPace();
  paced(pace, 50);
  assert.equal(pace.budget, MOVING_BUDGET / 2);
  for (let frame = 0; frame < 10; frame += 1) paced(pace, 50);
  assert.equal(pace.budget, MOVING_FLOOR);
});

test("thirty on-time frames double it back, never above MOVING_BUDGET", () => {
  const pace = newPace();
  paced(pace, 50);
  for (let frame = 0; frame < 29; frame += 1) paced(pace, 16.7);
  assert.equal(pace.budget, MOVING_BUDGET / 2);
  paced(pace, 16.7);
  assert.equal(pace.budget, MOVING_BUDGET);
  for (let frame = 0; frame < 60; frame += 1) paced(pace, 16.7);
  assert.equal(pace.budget, MOVING_BUDGET);
});

test("a frame that is neither late nor on time keeps the budget and restarts the run", () => {
  const pace = newPace();
  paced(pace, 50);
  for (let frame = 0; frame < 29; frame += 1) paced(pace, 16.7);
  paced(pace, 22);
  for (let frame = 0; frame < 29; frame += 1) paced(pace, 16.7);
  assert.equal(pace.budget, MOVING_BUDGET / 2);
});

test("only a flat canvas frame with more edges than the floor is paced", () => {
  assert.equal(worthPacing(MOVING_FLOOR + 1, false, true), true);
  assert.equal(worthPacing(MOVING_FLOOR, false, true), false);
  assert.equal(worthPacing(MOVING_FLOOR + 1, true, true), false);
  assert.equal(worthPacing(MOVING_FLOOR + 1, false, false), false);
});

// The HUD's throttle: how often the frame handler may write, with no timer of its own.
import assert from "node:assert/strict";
import { test } from "node:test";

import { due } from "../src/ui/throttle.ts";

const EVERY = 250;

test("a frame is due when the window has passed, and the first one (-Infinity) always is", () => {
  assert.equal(due(-Infinity, 0, EVERY), true);
  assert.equal(due(0, 1, EVERY), false);
  assert.equal(due(0, 249, EVERY), false);
  assert.equal(due(0, 250, EVERY), true);
  assert.equal(due(0, 4000, EVERY), true);
});

test("a clock that went backwards is not due: the frame waits for the next one", () => {
  assert.equal(due(1000, 900, EVERY), false);
  assert.equal(due(1000, 1250, EVERY), true);
});

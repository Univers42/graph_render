/** The hover fade: the dimmed opacity is reached 120 ms after the focus appears. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { FADE_MS, dimAt, fadeLevel } from "../src/fade.ts";

test("the fade lasts 120 ms", () => {
  assert.equal(FADE_MS, 120);
});

test("no focus is level 0, the start is 0, the middle 0.5, and past the end 1", () => {
  assert.equal(fadeLevel(-1, 500), 0);
  assert.equal(fadeLevel(1000, 1000), 0);
  assert.equal(fadeLevel(1000, 1060), 0.5);
  assert.equal(fadeLevel(1000, 1120), 1);
  assert.equal(fadeLevel(1000, 9000), 1);
});

test("a clock that ran backwards is level 0, never negative", () => {
  assert.equal(fadeLevel(1000, 900), 0);
});

test("the opacity runs from 1 to the theme's dim alpha", () => {
  assert.equal(dimAt(0.12, 0), 1);
  assert.equal(dimAt(0.12, 1), 0.12);
  assert.ok(Math.abs(dimAt(0.12, 0.5) - 0.56) < 1e-12);
});

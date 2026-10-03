// Where a moving frame redraws the kept GPU picture, and when it asks for a fresh one instead.
import assert from "node:assert/strict";
import { test } from "node:test";

import { landing } from "../src/webgl2/glide.ts";

const VIEWPORT = { width: 1000, height: 500 };

test("the same camera lands the picture where it was", () => {
  assert.deepEqual(landing([10, 20, 2], { x: 10, y: 20, scale: 2 }, VIEWPORT), { x: 0, y: 0, scale: 1 });
});

test("a zoom maps old screen pixels through the new camera", () => {
  const at = landing([100, 50, 1], { x: 80, y: 40, scale: 1.2 }, VIEWPORT);
  assert.ok(at !== null);
  // A world point at 0 sat at (100, 50); it now sits at (80, 40).
  assert.ok(Math.abs(100 * at.scale + at.x - 80) < 1e-9);
  assert.ok(Math.abs(50 * at.scale + at.y - 40) < 1e-9);
});

test("past the zoom limit either way, a fresh picture", () => {
  assert.equal(landing([0, 0, 1], { x: 0, y: 0, scale: 1.5 }, VIEWPORT), null);
  assert.equal(landing([0, 0, 1], { x: 0, y: 0, scale: 0.6 }, VIEWPORT), null);
});

test("a move that would bare more than a fifth of the view asks for a fresh picture", () => {
  assert.notEqual(landing([0, 0, 1], { x: 150, y: 0, scale: 1 }, VIEWPORT), null);
  assert.equal(landing([0, 0, 1], { x: 250, y: 0, scale: 1 }, VIEWPORT), null);
  // A centred zoom-out bares the rim: 0.9 squared keeps 81% covered, 0.85 squared 72%.
  assert.notEqual(landing([0, 0, 1], { x: 50, y: 25, scale: 0.9 }, VIEWPORT), null);
  assert.equal(landing([0, 0, 1], { x: 75, y: 37.5, scale: 0.85 }, VIEWPORT), null);
});

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DEFAULT_LIMITS, IDENTITY, centreOn, fitCamera, limitsFor, panBy, resetCamera, screenToWorld, worldToScreen, zoomAt,
} from "../src/camera.ts";

const VIEWPORT = { width: 800, height: 600 };

test("screen and world are inverse", () => {
  const camera = { x: 40, y: -12, scale: 1.7 };
  const back = screenToWorld(camera, worldToScreen(camera, { x: 3.5, y: -9 }));
  assert.ok(Math.abs(back.x - 3.5) < 1e-9 && Math.abs(back.y + 9) < 1e-9);
});

test("zooming keeps the world point under the cursor where it is", () => {
  const camera = { x: 10, y: 20, scale: 1 };
  const at = { x: 300, y: 200 };
  const before = screenToWorld(camera, at);
  const after = screenToWorld(zoomAt(camera, at, 1.8), at);
  assert.ok(Math.abs(before.x - after.x) < 1e-9 && Math.abs(before.y - after.y) < 1e-9);
});

test("zoom stops at the limits", () => {
  assert.equal(zoomAt(IDENTITY, { x: 0, y: 0 }, 1000).scale, DEFAULT_LIMITS.max);
  assert.equal(zoomAt(IDENTITY, { x: 0, y: 0 }, 0.0001).scale, DEFAULT_LIMITS.min);
  assert.equal(zoomAt(IDENTITY, { x: 0, y: 0 }, 0.0001, { min: 0.01, max: 2 }).scale, 0.01);
});

test("panning moves the offset and nothing else", () => {
  assert.deepEqual(panBy({ x: 1, y: 2, scale: 3 }, { x: 10, y: -5 }), { x: 11, y: -3, scale: 3 });
});

test("a fit centres the box and leaves the padding", () => {
  const camera = fitCamera({ minX: 0, minY: 0, maxX: 1344, maxY: 100 }, VIEWPORT);
  assert.equal(camera.scale, 0.5);
  assert.deepEqual(worldToScreen(camera, { x: 672, y: 50 }), { x: 400, y: 300 });
});

test("a fit of nothing is the identity camera", () => {
  assert.deepEqual(fitCamera(null, VIEWPORT), IDENTITY);
});

test("a fit never zooms a few nodes to the ceiling", () => {
  assert.equal(fitCamera({ minX: 0, minY: 0, maxX: 10, maxY: 10 }, VIEWPORT).scale, 2);
});

test("the zoom floor drops for a graph larger than the default floor shows", () => {
  const huge = { minX: 0, minY: 0, maxX: 80000, maxY: 60000 };
  const limits = limitsFor(huge, VIEWPORT);
  assert.ok(limits.min < DEFAULT_LIMITS.min);
  assert.ok(fitCamera(huge, VIEWPORT).scale >= limits.min);
  assert.deepEqual(limitsFor({ minX: 0, minY: 0, maxX: 100, maxY: 100 }, VIEWPORT), DEFAULT_LIMITS);
});

test("centring puts the world point in the middle of the viewport", () => {
  const camera = centreOn({ x: 0, y: 0, scale: 2 }, { x: 30, y: 40 }, VIEWPORT);
  assert.deepEqual(worldToScreen(camera, { x: 30, y: 40 }), { x: 400, y: 300 });
});

test("the zoom limits are the studio's own: 0.02 and 40", () => {
  assert.deepEqual(DEFAULT_LIMITS, { min: 0.02, max: 40 });
  assert.equal(zoomAt({ x: 0, y: 0, scale: 39.9 }, { x: 5, y: 5 }, 1.1).scale, 40);
  assert.equal(zoomAt({ x: 0, y: 0, scale: 0.021 }, { x: 5, y: 5 }, 0.1).scale, 0.02);
});

// A clamp moves the anchor: the scale stops, so the point under the cursor slides. The gate
// row is about the wheel inside the limits, which this pins: nothing moves under the cursor.
test("a zoom that is not clamped keeps the point under the cursor, at any scale", () => {
  const at = { x: 300, y: 200 };
  for (const scale of [0.02, 1, 7.5, 39.9]) {
    const before = screenToWorld({ x: 10, y: 20, scale }, at);
    const after = screenToWorld(zoomAt({ x: 10, y: 20, scale }, at, 1.2), at);
    assert.ok(Math.abs(before.x - after.x) < 1e-9 && Math.abs(before.y - after.y) < 1e-9, `at scale ${scale}`);
  }
});

// The offset follows the ratio the clamp actually applied, not the factor asked for: at 39.9
// asking ×9 gives ×1.002506…, so x is 300 - 290 × 1.002506… and y is 200 - 180 × 1.002506….
test("a clamped zoom anchors with the ratio it applied, not the factor asked for", () => {
  assert.deepEqual(zoomAt({ x: 10, y: 20, scale: 39.9 }, { x: 300, y: 200 }, 9), {
    scale: 40, x: 9.273182957393487, y: 19.548872180451127,
  });
});

test("a reset is 1:1 with the world origin in the middle of the viewport", () => {
  assert.deepEqual(resetCamera(VIEWPORT), { x: 400, y: 300, scale: 1 });
  assert.deepEqual(worldToScreen(resetCamera(VIEWPORT), { x: 0, y: 0 }), { x: 400, y: 300 });
});

test("a reset does not read the camera it replaces", () => {
  assert.deepEqual(resetCamera(VIEWPORT), resetCamera(VIEWPORT));
  assert.equal(resetCamera({ width: 100, height: 200 }).scale, 1);
});

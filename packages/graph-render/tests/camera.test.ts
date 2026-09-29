import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DEFAULT_LIMITS, IDENTITY, centreOn, fitCamera, limitsFor, panBy, screenToWorld, worldToScreen, zoomAt,
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

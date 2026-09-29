// Fit-to-view. The camera the studio opens every run with is this function's
// output, so its exact numbers are pinned: a changed padding, a dropped min
// extent or a swapped axis fails here rather than "looking slightly off".

import assert from "node:assert/strict";
import { test } from "node:test";

import { type NodeDraw, boundsOf, fitCamera } from "../src/core/fit.ts";
import { MAX_ZOOM, MIN_ZOOM } from "../../src/core/camera/transform.ts";

function node(index: number, x: number, y: number, w = 0, h = 0): NodeDraw {
  return { index, x, y, w, h, r: 0 };
}

test("boundsOf is the tight box over node centres when nothing has extent", () => {
  assert.deepEqual(boundsOf([node(0, 0, 0), node(1, 100, 40)]), {
    minX: 0,
    minY: 0,
    maxX: 100,
    maxY: 40,
  });
});

test("boundsOf grows by half a node's extent, so a Box is never clipped", () => {
  assert.deepEqual(boundsOf([node(0, 0, 0, 40, 20), node(1, 100, 0, 10, 30)]), {
    minX: -20,
    minY: -15,
    maxX: 105,
    maxY: 15,
  });
});

test("boundsOf of no nodes is null, not a zero box at the origin", () => {
  assert.equal(boundsOf([]), null);
});

test("a single node still frames, at the zoom ceiling", () => {
  const camera = fitCamera(boundsOf([node(0, 7, -3)]), 800, 600, 64);
  assert.equal(camera.scale, MAX_ZOOM);
  // The point lands dead centre of the viewport.
  assert.equal(camera.x + 7 * camera.scale, 400);
  assert.equal(camera.y + -3 * camera.scale, 300);
});

test("fitCamera centres the box and scales it to the padded viewport", () => {
  const bounds = boundsOf([node(0, 0, 0), node(1, 200, 100)]);
  const camera = fitCamera(bounds, 800, 600, 0);
  assert.equal(camera.scale, 4); // min(800/200, 600/100)
  assert.equal(camera.x, 400 - 100 * 4);
  assert.equal(camera.y, 300 - 50 * 4);
});

test("padding shrinks the usable box on both axes", () => {
  const bounds = boundsOf([node(0, 0, 0), node(1, 200, 100)]);
  const camera = fitCamera(bounds, 800, 600, 50);
  assert.equal(camera.scale, 3.5); // min(700/200, 500/100)
  assert.equal(camera.x, 400 - 100 * 3.5);
  assert.equal(camera.y, 300 - 50 * 3.5);
});

test("a degenerate box (one point, no extent) is framed at the zoom ceiling", () => {
  const camera = fitCamera({ minX: 5, minY: 5, maxX: 5, maxY: 5 }, 800, 600, 64);
  assert.equal(camera.scale, MAX_ZOOM);
  assert.ok(camera.scale <= MAX_ZOOM && camera.scale >= MIN_ZOOM);
});

test("a box with no extent is framed by the one-unit floor, not by its own coordinates", () => {
  // A viewport this small keeps both the right answer (3) and the wrong one (1)
  // inside the zoom clamps, so the unit floor is actually pinned rather than
  // hidden behind MAX_ZOOM: a divide-by-zero guard that never fires is not one.
  const camera = fitCamera({ minX: 2, minY: 2, maxX: 2, maxY: 2 }, 4, 3, 0);
  assert.equal(camera.scale, 3);
  assert.equal(camera.x, 2 - 2 * 3);
  assert.equal(camera.y, 1.5 - 2 * 3);
});

test("the default padding is 64 screen pixels on every side", () => {
  const bounds = boundsOf([node(0, 0, 0), node(1, 200, 100)]);
  assert.deepEqual(fitCamera(bounds, 800, 600), fitCamera(bounds, 800, 600, 64));
  assert.notDeepEqual(fitCamera(bounds, 800, 600), fitCamera(bounds, 800, 600, 0));
});

test("a huge box is clamped at the zoom floor rather than collapsing to zero", () => {
  const camera = fitCamera({ minX: 0, minY: 0, maxX: 1e9, maxY: 1e9 }, 800, 600, 64);
  assert.equal(camera.scale, MIN_ZOOM);
});

test("null bounds fall back to the identity camera", () => {
  assert.deepEqual(fitCamera(null, 800, 600), { x: 0, y: 0, scale: 1 });
});

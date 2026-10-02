/** The settled picture's textures: the size a driver can hold them at, and what cannot be held at all. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { pictureSize } from "../src/webgl2/picture.ts";

const AT_1080 = { viewport: { width: 1920, height: 1080 }, dpr: 1 };
const AT_1080_X2 = { viewport: { width: 1920, height: 1080 }, dpr: 2 };

test("a canvas a texture can hold is the canvas, in device pixels", () => {
  assert.deepEqual(pictureSize(AT_1080, 8192), [1920, 1080]);
  assert.deepEqual(pictureSize(AT_1080_X2, 8192), [3840, 2160]);
});

test("a canvas past the texture limit is scaled by its longer side, aspect kept", () => {
  // A 1080p canvas on a driver whose textures stop at 1024: half on the long side, half on the short.
  assert.deepEqual(pictureSize(AT_1080, 1024), [1024, 576]);
  // Device pixels, not CSS ones: at DPR 2 the canvas wants twice the texture, so the same limit
  // scales it half as far again.
  assert.deepEqual(pictureSize(AT_1080_X2, 1024), [1024, 576]);
});

test("a canvas over the limit on one side only is scaled by that side", () => {
  assert.deepEqual(pictureSize({ viewport: { width: 800, height: 4000 }, dpr: 1 }, 2048), [410, 2048]);
});

test("a degenerate canvas and a driver with no textures at all still give one texel each", () => {
  assert.deepEqual(pictureSize({ viewport: { width: 0, height: 0 }, dpr: 1 }, 8192), [1, 1]);
  assert.deepEqual(pictureSize(AT_1080, 0), [1, 1]);
});
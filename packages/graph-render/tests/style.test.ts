import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SIZING, plainStyle, radiusFor, styleFrom } from "../src/style.ts";

test("radius grows with the square root of weight", () => {
  const { base, gain } = DEFAULT_SIZING;
  assert.equal(radiusFor(0, DEFAULT_SIZING), base);
  assert.equal(radiusFor(1, DEFAULT_SIZING), base * (1 + gain));
  assert.equal(radiusFor(0.25, DEFAULT_SIZING), base * (1 + gain / 2));
});

test("a weight outside 0..1 or not a number is bounded, never drawn huge", () => {
  assert.equal(radiusFor(40, DEFAULT_SIZING), radiusFor(1, DEFAULT_SIZING));
  assert.equal(radiusFor(-3, DEFAULT_SIZING), radiusFor(0, DEFAULT_SIZING));
  assert.equal(radiusFor(Number.NaN, DEFAULT_SIZING), radiusFor(0, DEFAULT_SIZING));
});

test("every node is in exactly one colour bucket", () => {
  const style = styleFrom({
    labels: [], weights: new Float32Array(6), colours: Uint16Array.from([2, 0, 2, 1, 0, 2]), palette: ["a", "b", "c"],
  });
  assert.deepEqual([...style.bucketStart], [0, 2, 3, 6]);
  assert.deepEqual([...style.bucketItems], [1, 4, 3, 0, 2, 5]);
});

test("a colour past the palette lands in the last bucket", () => {
  const style = styleFrom({
    labels: [], weights: new Float32Array(2), colours: Uint16Array.from([9, 0]), palette: ["a", "b"],
  });
  assert.deepEqual([...style.bucketItems], [1, 0]);
});

test("rank is heaviest first, ties in index order", () => {
  const style = styleFrom({
    labels: [], weights: Float32Array.from([0.2, 0.9, 0.2, 1]), colours: new Uint16Array(4), palette: ["a"],
  });
  assert.deepEqual([...style.rank], [3, 1, 0, 2]);
  assert.equal(style.maxRadius, radiusFor(1, DEFAULT_SIZING));
});

test("the plain style has one bucket holding every node", () => {
  const style = plainStyle(3);
  assert.equal(style.palette.length, 1);
  assert.deepEqual([...style.bucketItems], [0, 1, 2]);
});

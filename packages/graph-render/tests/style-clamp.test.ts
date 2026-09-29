import assert from "node:assert/strict";
import { test } from "node:test";

import { styleFrom } from "../src/style.ts";

function radii(base: number, clamp: { min?: number; max?: number }): number[] {
  const style = styleFrom({
    labels: [], weights: Float32Array.from([0, 1]), colours: new Uint16Array(2), palette: ["a"],
    sizing: { base, gain: 2.5, ...clamp },
  });
  return [...style.radius];
}

test("no clamp: the radius is the formula", () => {
  assert.deepEqual(radii(4, {}).map((value) => Math.round(value * 100) / 100), [4, 14]);
});

test("a radius under the minimum or over the maximum is held there, and maxRadius follows", () => {
  assert.deepEqual(radii(0.8, { min: 1.5, max: 5 }).map((value) => Math.round(value * 100) / 100), [1.5, 2.8]);
  assert.deepEqual(radii(20, { min: 1.5, max: 30 }), [20, 30]);
  const style = styleFrom({
    labels: [], weights: Float32Array.from([1]), colours: new Uint16Array(1), palette: ["a"],
    sizing: { base: 20, gain: 2.5, max: 30 },
  });
  assert.equal(style.maxRadius, 30);
});

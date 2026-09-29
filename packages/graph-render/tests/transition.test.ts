import assert from "node:assert/strict";
import { test } from "node:test";

import { blend, easeInOutCubic } from "../src/transition.ts";

test("the easing starts at 0, ends at 1, is symmetric and never goes back", () => {
  assert.deepEqual([easeInOutCubic(0), easeInOutCubic(0.5), easeInOutCubic(1)], [0, 0.5, 1]);
  assert.deepEqual([easeInOutCubic(-4), easeInOutCubic(9)], [0, 1]);
  let previous = 0;
  for (let t = 0; t <= 1; t += 0.01) {
    assert.ok(easeInOutCubic(t) >= previous);
    assert.ok(Math.abs(easeInOutCubic(t) + easeInOutCubic(1 - t) - 1) < 1e-9);
    previous = easeInOutCubic(t);
  }
});

test("a blend is the start at 0, the end at 1, and writes only the output", () => {
  const from = Float32Array.from([0, 10]);
  const to = Float32Array.from([8, -10]);
  const out = new Float32Array(2);
  blend(from, to, 0.25, out);
  assert.deepEqual([...out], [2, 5]);
  blend(from, to, 1, out);
  assert.deepEqual([...out], [...to]);
  assert.deepEqual([...from], [0, 10]);
});

// The 600 ms layout-to-layout transition. Every number here is pinned: the
// duration, the easing curve's exact values, and the rule that decides whether a
// polyline/curve path can be interpolated at all.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  TRANSITION_MS,
  clamp01,
  easeInOutCubic,
  interpolatePath,
  lerpPositions,
  transitionProgress,
} from "../src/core/transition.ts";

function f32(values: number[]): Float32Array {
  return new Float32Array(values);
}

test("the transition is 600 ms", () => {
  assert.equal(TRANSITION_MS, 600);
});

test("clamp01 pins the unit interval from both ends", () => {
  assert.equal(clamp01(-3), 0);
  assert.equal(clamp01(0), 0);
  assert.equal(clamp01(0.25), 0.25);
  assert.equal(clamp01(1), 1);
  assert.equal(clamp01(9), 1);
});

test("easeInOutCubic is the pinned curve: slow start, fast middle, slow end", () => {
  assert.equal(easeInOutCubic(0), 0);
  assert.equal(easeInOutCubic(0.25), 0.0625);
  assert.equal(easeInOutCubic(0.5), 0.5);
  assert.equal(easeInOutCubic(0.75), 0.9375);
  assert.equal(easeInOutCubic(1), 1);
});

test("transitionProgress is linear in time and saturates at the duration", () => {
  assert.equal(transitionProgress(0), 0);
  assert.equal(transitionProgress(150), 0.25);
  assert.equal(transitionProgress(300), 0.5);
  assert.equal(transitionProgress(600), 1);
  assert.equal(transitionProgress(5000), 1);
  assert.equal(transitionProgress(-10), 0);
});

test("transitionProgress accepts an explicit duration", () => {
  assert.equal(transitionProgress(50, 100), 0.5);
  assert.equal(transitionProgress(50, 0), 1);
});

test("lerpPositions interpolates every element, endpoints included", () => {
  const from = f32([0, 10, -4]);
  const to = f32([10, 0, 4]);
  assert.deepEqual([...lerpPositions(from, to, 0)], [0, 10, -4]);
  assert.deepEqual([...lerpPositions(from, to, 0.5)], [5, 5, 0]);
  assert.deepEqual([...lerpPositions(from, to, 1)], [10, 0, 4]);
});

test("lerpPositions writes into a caller-supplied buffer, so the draw loop allocates nothing", () => {
  const out = f32([0, 0, 0]);
  const returned = lerpPositions(f32([0, 0, 0]), f32([4, 8, 12]), 0.5, out);
  assert.equal(returned, out);
  assert.deepEqual([...out], [2, 4, 6]);
});

test("lerpPositions refuses buffers of different lengths instead of truncating", () => {
  assert.throws(() => lerpPositions(f32([0, 1]), f32([0, 1, 2]), 0.5), /same length/);
});

test("interpolatePath lerps a path whose point count matches", () => {
  const out = interpolatePath(f32([0, 0, 10, 10]), f32([10, 0, 10, 20]), 0.5);
  assert.deepEqual(out === null ? null : [...out], [5, 0, 10, 15]);
});

test("interpolatePath returns null when the two paths cannot be matched point for point", () => {
  assert.equal(interpolatePath(f32([0, 0, 1, 1]), f32([0, 0, 1, 1, 2, 2]), 0.5), null);
  assert.equal(interpolatePath(new Float32Array(0), new Float32Array(0), 0.5), null);
});

test("interpolatePath at t=0 and t=1 returns the endpoints themselves", () => {
  assert.deepEqual([...(interpolatePath(f32([1, 2]), f32([3, 4]), 0) ?? [])], [1, 2]);
  assert.deepEqual([...(interpolatePath(f32([1, 2]), f32([3, 4]), 1) ?? [])], [3, 4]);
});

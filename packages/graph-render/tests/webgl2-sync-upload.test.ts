/**
 * The upload policy of the GPU layer: a column that fits the buffer it is going into is written
 * into it, and a column that outgrows its buffer is the only thing that reallocates.
 *
 * WHY the policy is a class and not a line inside `syncNodes`: node has no WebGL2 context, and
 * a per-frame driver allocation is invisible to every other test in the package — a buffer that
 * was thrown away and refilled with the same bytes draws exactly the same drawing. Testing the
 * decision on its own is the only way to assert "no reallocation" rather than "still looks
 * right", and keeping it free of `WebGLBuffer` is what lets this test run at all.
 *
 * The control is the last test: a column one element longer than the buffer must ask for a new
 * allocation, so the suite cannot pass by never writing anything.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { Sizes } from "../src/webgl2/sync.ts";

// One f32 column of a million nodes, and of a million and one.
const MILLION = 4_000_000;

test("a buffer nobody has written to needs an allocation", () => {
  const sizes = new Sizes<string>();
  assert.equal(sizes.needsAlloc("x", MILLION), true);
});

test("the first column allocates, and then a same-size column is only written into", () => {
  const sizes = new Sizes<string>();
  assert.equal(sizes.needsAlloc("x", MILLION), true, "an empty buffer has to be allocated");
  sizes.record("x", MILLION);
  assert.equal(sizes.needsAlloc("x", MILLION), false, "and then the same size needs no allocation");
});

test("a shrinking column is still written into the buffer it already has", () => {
  const sizes = new Sizes<string>();
  sizes.record("x", MILLION);
  assert.equal(sizes.needsAlloc("x", 4_000), false);
  assert.equal(sizes.needsAlloc("x", 0), false, "even an empty one fits");
});

test("the control: a column longer than its buffer does ask for a new allocation", () => {
  const sizes = new Sizes<string>();
  sizes.record("x", MILLION);
  assert.equal(sizes.needsAlloc("x", MILLION + 4), true, "one element more does not fit");
});

test("the control: one buffer's size says nothing about another's", () => {
  const sizes = new Sizes<string>();
  sizes.record("x", MILLION);
  assert.equal(sizes.needsAlloc("y", 4_000), true, "y has never been written to");
  assert.equal(sizes.needsAlloc("x", MILLION), false);
});

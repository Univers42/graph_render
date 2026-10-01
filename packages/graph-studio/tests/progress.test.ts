// The progress bar's state machine: the settle empties as alpha cools, a settled graph
// hides the bar, a batch run shows the same bar with no fraction of its own.
import assert from "node:assert/strict";
import { test } from "node:test";

import { ALPHA_MIN, HIDDEN, batchBar, settleBar } from "../src/ui/progress.ts";

test("a hot simulation is a full bar, a cold one is empty, and both are shown", () => {
  assert.equal(settleBar(1, true).visible, true);
  assert.equal(settleBar(1, true).fraction, 1);
  assert.equal(settleBar(ALPHA_MIN, true).fraction, 0);
  // Linear in alpha over [ALPHA_MIN, 1], so the midpoint of the range is not alpha 0.5.
  const middle = (0.5 - ALPHA_MIN) / (1 - ALPHA_MIN);
  assert.equal(settleBar(0.5, true).fraction, middle);
  const cold = settleBar(0.2, true).fraction;
  const warm = settleBar(0.6, true).fraction;
  assert.ok(typeof cold === "number" && typeof warm === "number" && cold < warm, "less alpha, less bar");
});

test("the fraction is clamped: an alpha under alpha_min, or over one, is still 0..1", () => {
  assert.equal(settleBar(0, true).fraction, 0);
  assert.equal(settleBar(-1, true).fraction, 0);
  assert.equal(settleBar(4, true).fraction, 1);
});

test("alpha that is not a number is a hidden bar, not a NaN width", () => {
  assert.deepEqual(settleBar(Number.NaN, true), HIDDEN);
  assert.deepEqual(settleBar(Number.POSITIVE_INFINITY, true), HIDDEN);
});

test("a settled graph hides the bar", () => {
  assert.deepEqual(settleBar(0.5, false), HIDDEN);
  assert.deepEqual(settleBar(1, false), HIDDEN, "a reheated graph that is not running is not settling");
});

test("a batch run shows the same bar with no fraction of its own", () => {
  const busy = batchBar(2);
  assert.equal(busy.visible, true);
  assert.equal(busy.fraction, null, "one layout call reports no progress of its own");
  assert.match(busy.label, /2/);
  assert.deepEqual(batchBar(0), HIDDEN);
  assert.deepEqual(batchBar(-1), HIDDEN);
});

test("the label says what is running, and the settled bar says nothing", () => {
  assert.equal(settleBar(0.4, true).label, "settling");
  assert.equal(HIDDEN.label, "");
});
/** The still picture's own report: what the view's stats say about the fill that is under way. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { newCounts } from "../src/canvas2d/input.ts";
import { newRate } from "../src/canvas2d/rate.ts";
import { newBulkSlot } from "../src/webgl2/hook.ts";
import { refiningOf } from "../src/webgl2/still.ts";
import { statsOf } from "../src/view-stats.ts";

/** The loop fields `statsOf` reads, with an empty scene and the bulk slot's own `refining`. */
function state(refining: boolean) {
  const slot = newBulkSlot("webgl2");
  slot.refining = refining;
  return {
    counts: newCounts(), bulk: slot, rate: newRate(), sprites: { rasterised: () => 0 },
    layoutRuns: 0, frameMs: 0, frames: 0, scene: { frame: { nodeCount: 3, edgeCount: 2 } },
  };
}

test("chunks still missing mean the picture is refining, a full or lost picture is not", () => {
  // One pair left is still refining: the loop owes one more frame for it.
  assert.equal(refiningOf(1), true);
  assert.equal(refiningOf(2048), true);
  // Nothing left is the frame that completed the picture.
  assert.equal(refiningOf(0), false);
  // -1 is the lost context (paintStill): the layer is gone, so nothing is being filled.
  assert.equal(refiningOf(-1), false);
});

test("the stats carry the flag, true while chunks remain and false once the picture is full", () => {
  assert.equal(statsOf(state(true)).refining, true);
  assert.equal(statsOf(state(false)).refining, false);
});

test("the stats carry the edge draw's GPU milliseconds, and none while no layer counted", () => {
  // No layer was ever made, so nothing has been timed: 0, not undefined.
  assert.equal(statsOf(state(false)).gpuEdgeMs, 0);
  // A layer that counted reads through the slot's own reader (webgl2/hook.ts layerOf).
  const withLayer = state(false);
  withLayer.bulk.gpuEdgeMs = () => 12.5;
  assert.equal(statsOf(withLayer).gpuEdgeMs, 12.5);
});
// The frame after a move: a frame that drew less than the whole set owes one settled frame,
// or what it drew less of stays on screen. The edge gradient is one such case: while the view
// moves its mixed edges take their mean colour, and the gradient comes back only on the frame
// the view has stopped for. A frame the GPU layer drew is the other: it was a paced sample of
// the set or a glide over an older picture (webgl2/hook.ts), whatever the set's size.
import assert from "node:assert/strict";
import { test } from "node:test";

import { drewAWay } from "../src/canvas2d/loop.ts";

test("a frame whose mixed edges took their mean owes a settled frame", () => {
  assert.equal(drewAWay({ mixedEdges: 120, gradientStrokes: 0, bulk: 0 }), true);
});

test("a frame that drew every mixed edge its gradient owes nothing", () => {
  assert.equal(drewAWay({ mixedEdges: 120, gradientStrokes: 120, bulk: 0 }), false);
});

test("a frame with no mixed edge at all owes nothing", () => {
  assert.equal(drewAWay({ mixedEdges: 0, gradientStrokes: 0, bulk: 0 }), false);
});

test("a moving frame the GPU layer drew owes a settled frame", () => {
  // 5000 nodes and 14 997 edges left the last moving frame's 2048-node sample on screen for good.
  assert.equal(drewAWay({ mixedEdges: 0, gradientStrokes: 0, bulk: 2 }), true);
});

// The frame after a move: a frame that drew less than the whole set owes one settled frame,
// or what it drew less of stays on screen. The edge gradient is the second such case: while
// the view moves its mixed edges take their mean colour, and the gradient comes back only on
// the frame the view has stopped for.
import assert from "node:assert/strict";
import { test } from "node:test";

import { drewAWay } from "../src/canvas2d/loop.ts";

test("a frame whose mixed edges took their mean owes a settled frame", () => {
  assert.equal(drewAWay({ mixedEdges: 120, gradientStrokes: 0 }), true);
});

test("a frame that drew every mixed edge its gradient owes nothing", () => {
  assert.equal(drewAWay({ mixedEdges: 120, gradientStrokes: 120 }), false);
});

test("a frame with no mixed edge at all owes nothing", () => {
  assert.equal(drewAWay({ mixedEdges: 0, gradientStrokes: 0 }), false);
});
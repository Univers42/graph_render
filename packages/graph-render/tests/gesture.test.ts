/** What a press, a wheel and a double-click mean, read without a browser. */
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DOUBLE_CLICK_ZOOM, PINCH_ZOOM, centreOf, dragKindOf, isClick, isDrag, travelledBy, wheelFactor, wheelPixels,
} from "../src/gesture.ts";

test("the left button drags the graph, and the middle button always pans", () => {
  assert.equal(dragKindOf(0, false), "select");
  assert.equal(dragKindOf(1, false), "pan");
  assert.equal(dragKindOf(1, true), "pan");
  assert.equal(dragKindOf(2, false), "none", "the right button is the host's menu");
  assert.equal(dragKindOf(0, true), "pan", "space turns a left drag into a pan");
  assert.equal(dragKindOf(2, true), "none");
});

test("a pinch is a much smaller delta than a wheel notch, and both zoom in", () => {
  const wheel = wheelFactor(-100, false);
  const pinch = wheelFactor(-100, true);
  assert.ok(wheel > 1 && pinch > 1, "a negative delta zooms in");
  assert.ok(pinch > wheel, "the same delta pinches further");
  assert.equal(PINCH_ZOOM, 0.012);
  assert.equal(wheelFactor(100, false), 1 / wheelFactor(-100, false), "a notch down undoes it");
});

test("a line-mode wheel is read in pixels, and a pixel-mode one is left alone", () => {
  assert.equal(wheelPixels(3, 1, 16), 48);
  assert.equal(wheelPixels(3, 0, 16), 3);
  assert.equal(wheelPixels(-3, 0, 16), -3);
});

test("travel is the taxicab sum, and four pixels make a press a drag", () => {
  assert.equal(travelledBy(3, 4, 0), 7);
  assert.equal(travelledBy(-2, 2, 7), 11);
  assert.equal(isDrag(4), false, "exactly the slop is still a click");
  assert.equal(isDrag(5), true);
  assert.equal(isClick(4), true);
  assert.equal(isClick(5), false);
});

test("a double-click zooms in by two at the cursor", () => {
  assert.equal(DOUBLE_CLICK_ZOOM, 2);
});

test("the centre of a viewport is half of it", () => {
  assert.deepEqual(centreOf({ width: 800, height: 600 }), { x: 400, y: 300 });
  assert.deepEqual(centreOf({ width: 1, height: 1 }), { x: 0.5, y: 0.5 });
});

/** The live drag: press pins, move follows, up or cancel releases; disabled leaves the view-only drag. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { liveGesture, type LiveDrag } from "../src/drag.ts";
import type { Point } from "../src/camera.ts";

function recorder(enabled = true) {
  const calls: string[] = [];
  const port: LiveDrag = {
    enabled: () => enabled,
    drag: (node, at) => calls.push(`drag ${node} ${at.x},${at.y}`),
    release: (node) => calls.push(`release ${node}`),
  };
  return { calls, port };
}

function held(port: LiveDrag, node: number, from: Point) {
  const gesture = liveGesture(port, node, doubled, from);
  if (gesture === null) throw new Error("expected a live gesture");
  return gesture;
}

const doubled = (at: Point): Point => ({ x: at.x * 2, y: at.y * 2 });

test("press sends the world position, move follows, end releases once", () => {
  const { calls, port } = recorder();
  const gesture = held(port, 4, { x: 1, y: 1 });
  gesture.move({ x: 5, y: 6 });
  gesture.end({ x: 7, y: 8 });
  gesture.end({ x: 7, y: 8 });
  assert.deepEqual(calls, ["drag 4 2,2", "drag 4 10,12", "release 4"]);
});

test("cancel releases, and nothing follows it", () => {
  const { calls, port } = recorder();
  const gesture = held(port, 2, { x: 0, y: 0 });
  gesture.cancel();
  gesture.move({ x: 9, y: 9 });
  gesture.end({ x: 9, y: 9 });
  assert.deepEqual(calls, ["drag 2 0,0", "release 2"]);
});

test("a second gesture on the same port while one is held is ignored", () => {
  const { calls, port } = recorder();
  const first = held(port, 1, { x: 0, y: 0 });
  const second = liveGesture(port, 3, doubled, { x: 1, y: 1 });
  assert.equal(second, null);
  first.end({ x: 0, y: 0 });
  assert.deepEqual(calls, ["drag 1 0,0", "release 1"]);
});

test("a disabled port yields no live gesture", () => {
  const { calls, port } = recorder(false);
  assert.equal(liveGesture(port, 0, doubled, { x: 0, y: 0 }), null);
  assert.deepEqual(calls, []);
});

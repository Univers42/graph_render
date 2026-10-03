/** The live drag: a move pins, up or cancel releases; disabled leaves the view-only drag. */
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

function held(port: LiveDrag, node: number) {
  const gesture = liveGesture(port, node, doubled);
  if (gesture === null) throw new Error("expected a live gesture");
  return gesture;
}

const doubled = (at: Point): Point => ({ x: at.x * 2, y: at.y * 2 });

test("move sends the world position, end releases once", () => {
  const { calls, port } = recorder();
  const gesture = held(port, 4);
  gesture.move({ x: 5, y: 6 });
  gesture.end({ x: 7, y: 8 });
  gesture.end({ x: 7, y: 8 });
  assert.deepEqual(calls, ["drag 4 10,12", "release 4"]);
});

test("press sends nothing yet: a press that never travels is a click, and it pins nothing", () => {
  // The click path never calls `end`, so a pin taken on press would outlive the press.
  const { calls, port } = recorder();
  const gesture = held(port, 4);
  assert.deepEqual(calls, [], "the motor is not told about a press until the pointer moves");
  gesture.cancel();
  assert.deepEqual(calls, [], "and a press that went nowhere releases nothing either");
});

test("cancel releases a drag that started, and nothing follows it", () => {
  const { calls, port } = recorder();
  const gesture = held(port, 2);
  gesture.move({ x: 1, y: 1 });
  gesture.cancel();
  gesture.move({ x: 9, y: 9 });
  gesture.end({ x: 9, y: 9 });
  assert.deepEqual(calls, ["drag 2 2,2", "release 2"]);
});

test("a second gesture on the same port while one is held is ignored", () => {
  const { calls, port } = recorder();
  const first = held(port, 1);
  const second = liveGesture(port, 3, doubled);
  assert.equal(second, null, "the port is reserved on press, before the pointer has moved");
  first.move({ x: 2, y: 2 });
  first.end({ x: 2, y: 2 });
  assert.deepEqual(calls, ["drag 1 4,4", "release 1"]);
});

test("a disabled port yields no live gesture", () => {
  const { calls, port } = recorder(false);
  assert.equal(liveGesture(port, 0, doubled), null);
  assert.deepEqual(calls, []);
});

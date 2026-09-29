import assert from "node:assert/strict";
import { test } from "node:test";

import { TARGET_SPACING, frameFrom, worldFactor } from "../src/frame.ts";
import { decodeSnapshot } from "../src/snapshot/decode.ts";

const PINNED = Uint8Array.from([
  0x47, 0x4d, 0x53, 0x4e, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0,
  0, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 0x61, 0x62, 0x63, 0,
  0, 0, 0, 0, 1, 0, 0, 0, 0x65, 0, 0, 0,
  0, 0, 0, 0, 1, 0, 0, 0,
  0, 0, 0x80, 0x3f, 0, 0, 0x20, 0xc0, 0, 0, 0, 0, 0, 0, 0, 0x3f,
  0, 0, 0, 0,
]);

test("a uniform spread is brought to the target spacing", () => {
  const factor = worldFactor({ minX: 0, minY: 0, maxX: 10, maxY: 10 }, 100);
  assert.equal(factor, TARGET_SPACING);
});

test("a collinear layout falls back to extent over gaps", () => {
  assert.equal(worldFactor({ minX: 0, minY: 5, maxX: 9, maxY: 5 }, 10), TARGET_SPACING);
});

test("no spread means no scaling", () => {
  assert.equal(worldFactor({ minX: 3, minY: 3, maxX: 3, maxY: 3 }, 4), 1);
  assert.equal(worldFactor(null, 0), 1);
  assert.equal(worldFactor({ minX: 0, minY: 0, maxX: 1, maxY: 1 }, 1), 1);
});

test("a frame is the snapshot scaled by one factor, in its own memory", () => {
  const bytes = PINNED.slice();
  const frame = frameFrom(decodeSnapshot(bytes));
  assert.deepEqual([frame.nodeCount, frame.edgeCount], [2, 1]);
  assert.ok(Math.abs((frame.x[0] ?? 0) / frame.factor - 1) < 1e-6);
  assert.ok(Math.abs((frame.x[1] ?? 0) / frame.factor + 2.5) < 1e-6);
  assert.deepEqual(frame.bounds, { minX: frame.x[1], minY: frame.y[0], maxX: frame.x[0], maxY: frame.y[1] });
  const before = [...frame.x, ...frame.source, ...frame.target];
  bytes.fill(0);
  assert.deepEqual([...frame.x, ...frame.source, ...frame.target], before);
});

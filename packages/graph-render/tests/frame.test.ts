import assert from "node:assert/strict";
import { test } from "node:test";

import { fitCamera, worldToScreen } from "../src/camera.ts";
import { type Frame, TARGET_SPACING, frameFrom, worldFactor } from "../src/frame.ts";
import { decodeSnapshot } from "../src/snapshot/decode.ts";
import { column, stringTable } from "./support.ts";

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

/** u32 columns as bytes: the endpoints and a polyline's offsets. */
function words(values: readonly number[]): number[] {
  return [...new Uint8Array(Uint32Array.from(values).buffer)];
}

interface BoxSpec {
  readonly x: readonly number[];
  readonly y: readonly number[];
  /** Interior edge vertices as [x, y] pairs; `null` for a Line snapshot, which has none. */
  readonly pts: readonly (readonly [number, number])[] | null;
}

/**
 * Real bytes for a `Box` snapshot with one routed edge: node tag 2, edge tag 1, dim 0, and
 * the w/h size columns a Box carries, then the offsets and the interior points. Built here
 * because `support.spaceBytes` writes a Circle/Line frame with no interior points, and a
 * hand-built `Frame` would test the painter against this file's idea of a column order.
 */
function boxBytes(spec: BoxSpec): Uint8Array {
  const nodes = spec.x.length;
  const routed = spec.pts !== null;
  const pts = spec.pts ?? [];
  const edges = routed ? 1 : 0;
  const header = new Uint8Array(Uint32Array.of(0x4e534d47, 0, 0, 1, 1, nodes, edges).buffer);
  header[12] = 2;
  header[13] = routed ? 1 : 0;
  const parts = [
    [...header],
    stringTable(Array.from({ length: nodes }, (_, at) => `n${at}`)),
    stringTable(edges === 0 ? [] : ["e0"]),
    edges === 0 ? [] : words([0]),
    edges === 0 ? [] : words([nodes - 1]),
    column(spec.x),
    column(spec.y),
    column(Array.from({ length: nodes }, () => 8)),
    column(Array.from({ length: nodes }, () => 6)),
  ];
  if (routed) parts.push(words([0, pts.length]), column(pts.flat()));
  return Uint8Array.from(parts.flat());
}

/** A two-node Box frame at x = 100..400 whose one polyline swings out to +-5000 and 9000. */
function routedFrame(): Frame {
  return frameFrom(decodeSnapshot(boxBytes({ x: [100, 400], y: [100, 400], pts: [[-5000, 9000], [9000, -5000]] })));
}

test("a frame with no interior edge points is bounded by its nodes, exactly as before", () => {
  const frame = frameFrom(decodeSnapshot(boxBytes({ x: [100, 400], y: [100, 400], pts: null })));
  assert.equal(frame.nodeKind, "Box");
  assert.equal(frame.pts, null);
  assert.deepEqual(frame.bounds, {
    minX: frame.x[0], minY: frame.y[0], maxX: frame.x[1], maxY: frame.y[1],
  });
  // The world factor is the node hull's spacing, and the edge columns never entered it.
  assert.equal(frame.factor, worldFactor({ minX: 100, minY: 100, maxX: 400, maxY: 400 }, 2));
});

test("a routed vertex outside the node hull is in the bounds a fit uses", () => {
  const frame = routedFrame();
  const pts = frame.pts ?? new Float32Array(0);
  const bounds = frame.bounds;
  assert.notEqual(bounds, null);
  // The scale of the drawing is still the node hull's: widening the bounds does not rescale
  // the layout, so the far vertex sits at 9000 * this factor and no nearer.
  assert.equal(frame.factor, worldFactor({ minX: 100, minY: 100, maxX: 400, maxY: 400 }, 2));
  assert.ok((pts[2] ?? 0) > (frame.x[0] ?? 0), "the vertex is outside the node hull");
  assert.ok((bounds?.maxX ?? 0) >= (pts[2] ?? 0), `maxX ${bounds?.maxX} is short of the vertex at ${pts[2]}`);
  assert.ok((bounds?.maxY ?? 0) >= (pts[1] ?? 0), `maxY ${bounds?.maxY} is short of the vertex at ${pts[1]}`);
  assert.ok((bounds?.minX ?? 0) <= (pts[0] ?? 0), `minX ${bounds?.minX} is past the vertex at ${pts[0]}`);
  assert.ok((bounds?.minY ?? 0) <= (pts[3] ?? 0), `minY ${bounds?.minY} is past the vertex at ${pts[3]}`);
});

test("a fit over a routed drawing keeps every interior vertex on screen", () => {
  const viewport = { width: 1200, height: 1200 };
  const frame = routedFrame();
  const camera = fitCamera(frame.bounds, viewport);
  const pts = frame.pts ?? new Float32Array(0);
  for (const at of [0, 2]) {
    const screen = worldToScreen(camera, { x: pts[at] ?? 0, y: pts[at + 1] ?? 0 });
    assert.ok(screen.x >= 0 && screen.x <= viewport.width, `vertex x ${screen.x} is off the frame`);
    assert.ok(screen.y >= 0 && screen.y <= viewport.height, `vertex y ${screen.y} is off the frame`);
  }
});

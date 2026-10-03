/**
 * A moved scene: the positions are handed over eagerly, the bounds and the pick grid are not.
 * A frame nobody picks in and nobody fits reads no column at all.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Positions, gridOf } from "../src/grid.ts";
import { EMPTY_FRAME, type Scene, boundsOf, pickIn, sceneOf } from "../src/scene.ts";
import { plainStyle } from "../src/style.ts";
import { deferredScene } from "../src/lazy.ts";
import { movedScene } from "../src/drag.ts";

function threeNodes() {
  const x = Float32Array.from([0, 100, 200]);
  const y = Float32Array.from([0, 0, 0]);
  const frame = {
    ...EMPTY_FRAME, nodeCount: 3, edgeCount: 1, x, y,
    source: Uint32Array.from([0]), target: Uint32Array.from([1]),
    bounds: { minX: 0, minY: 0, maxX: 200, maxY: 0 },
  };
  return sceneOf(frame, plainStyle(3), null);
}

/**
 * `Float32Array` that counts the column reads a bounds scan or a grid build makes. The nodes
 * are spread along a line, so a pick at one of them names exactly that node.
 */
function countedPositions(count: number): { positions: Positions; reads: () => number } {
  let reads = 0;
  const xs = new Float32Array(count);
  const ys = new Float32Array(count);
  for (let i = 0; i < count; i += 1) {
    xs[i] = i * 10;
    ys[i] = 0;
  }
  // `Reflect.get` with the proxy as receiver would run the TypedArray length getter on the
  // proxy, which it refuses; the target is the receiver instead.
  const watch = (values: Float32Array): Float32Array => new Proxy(values, {
    get: (target, key): unknown => {
      reads += 1;
      const slot: unknown = Reflect.get(target, key);
      return slot;
    },
  });
  return { positions: { x: watch(xs), y: watch(ys) }, reads: () => reads };
}

const HERE = { tolerance: 1, floor: 4 };

test("the deferred scene draws from the columns it was handed, and keeps the rest of the scene", () => {
  const scene = threeNodes();
  const x = Float32Array.from([5, 150, 200]);
  const y = Float32Array.from([0, 30, 0]);
  const moved = deferredScene(scene, { x, y });
  assert.equal(moved.frame.x, x);
  assert.equal(moved.frame.y, y);
  assert.equal(moved.style, scene.style);
  assert.equal(moved.adjacency, scene.adjacency);
  assert.equal(moved.extent, scene.extent);
  assert.equal(moved.reach, scene.reach);
  assert.equal(moved.frame.source, scene.frame.source, "the edge ends are the layout's");
  assert.equal(scene.frame.x[0], 0, "the frame the motor handed over is never written to");
});

test("the bounds are computed on the first read, then reused, and are the moved ones", () => {
  const scene = threeNodes();
  const x = Float32Array.from([5, 150, 200]);
  const y = Float32Array.from([0, 30, 0]);
  const moved = deferredScene(scene, { x, y });
  const bounds = moved.bounds;
  assert.deepEqual(bounds, boundsOf({ x, y }, scene.reach));
  assert.equal(moved.bounds, bounds, "a second read is the same object");
  assert.equal(scene.reach, 4, "the plain style's radius is the reach");
  assert.deepEqual(bounds, { minX: 1, minY: -4, maxX: 204, maxY: 34 }, "grown by the reach, so a fit shows whole nodes");
});

test("the grid is computed on the first read, then reused, over the bounds just computed", () => {
  const scene = threeNodes();
  const x = Float32Array.from([5, 150, 200]);
  const y = Float32Array.from([0, 30, 0]);
  const moved = deferredScene(scene, { x, y });
  const grid = moved.grid;
  assert.equal(moved.grid, grid, "a second read is the same object");
  assert.deepEqual(grid, gridOf({ x, y }, boundsOf({ x, y }, scene.reach)));
  assert.equal(pickIn(moved, { x: 150, y: 30, ...HERE }), 1, "the grid and the columns agree");
});

test("building the scene reads no column: the bounds and the grid are not computed yet", () => {
  const scene = threeNodes();
  const { positions, reads } = countedPositions(3);
  const moved = deferredScene(scene, positions);
  assert.equal(reads(), 0, "an eager build would have read the bounds already");
  const bounds = moved.bounds;
  assert.ok(reads() > 0, "reading the bounds is what scans the frame");
  const after = reads();
  assert.equal(moved.bounds, bounds);
  assert.equal(reads(), after, "and only the first read scans it");
});

test("a 200k-node move costs no grid: nothing is scanned until something picks", () => {
  const scene = threeNodes();
  const { positions, reads } = countedPositions(200_000);
  const moved = deferredScene(scene, positions);
  assert.equal(reads(), 0, "the frame is untouched by the move");
  assert.deepEqual(moved.bounds, { minX: -4, minY: -4, maxX: 1999994, maxY: 4 }, "the scan, when asked for");
  assert.equal(moved.grid.items.length, 200_000, "the grid, when asked for, covers every node");
  assert.equal(pickIn(moved, { x: 1000, y: 0, ...HERE }), 100, "and the grid answers over all of them");
});

/** The descriptor of one own property, so a missing one is a failure and not a silent skip. */
function descriptorOf(scene: Scene, key: string): PropertyDescriptor {
  const found: PropertyDescriptor | undefined = Object.getOwnPropertyDescriptor(scene, key);
  if (found === undefined) throw new Error(`${key} is not an own property of the scene`);
  return found;
}

test("the deferred scene still behaves like a plain object for anything that iterates it", () => {
  const moved = deferredScene(threeNodes(), { x: Float32Array.from([1, 2, 3]), y: Float32Array.from([0, 0, 0]) });
  const keys = Object.keys(moved).sort();
  assert.deepEqual(keys, ["adjacency", "bounds", "extent", "frame", "grid", "reach", "style"]);
  for (const key of keys) {
    assert.equal(descriptorOf(moved, key).configurable, true, `${key} must stay configurable`);
    assert.equal(descriptorOf(moved, key).enumerable, true, `${key} must stay enumerable`);
  }
  // The shape of the laziness: two accessors, everything else a plain data property.
  assert.equal(typeof descriptorOf(moved, "grid").get, "function");
  assert.equal(typeof descriptorOf(moved, "bounds").get, "function");
  assert.equal(descriptorOf(moved, "frame").value, moved.frame);
  assert.deepEqual(Object.entries({ ...moved }).map(([key]) => key).sort(), keys, "a spread sees every field");
});

test("movedScene matches the eager bounds and grid it used to build", () => {
  const scene = threeNodes();
  const x = Float32Array.from([5, 150, 200]);
  const y = Float32Array.from([0, 30, 0]);
  const moved = movedScene(scene, { x, y });
  assert.deepEqual(moved.bounds, boundsOf({ x, y }, scene.reach));
  assert.deepEqual(moved.grid, gridOf({ x, y }, boundsOf({ x, y }, scene.reach)));
  assert.deepEqual(moved.frame, { ...scene.frame, x, y });
});

test("movedScene reads no column either, and one pick builds the grid the next one reuses", () => {
  const scene = threeNodes();
  const { positions, reads } = countedPositions(200_000);
  const moved = movedScene(scene, positions);
  assert.equal(reads(), 0, "a per-frame move must not scan");
  assert.equal(pickIn(moved, { x: 1000, y: 0, ...HERE }), 100, "the pick finds the moved node");
  const built = reads();
  assert.ok(built > 200_000, "the first pick paid for the grid over every node");
  assert.equal(pickIn(moved, { x: 1000, y: 0, ...HERE }), 100);
  // The pick reads only the nodes it tests; the frame was scanned once, not twice.
  assert.ok(reads() - built < 20, "the next pick reuses the grid instead of rebuilding it");
});

test("a scene of no nodes has no bounds and an empty grid, lazily as ever", () => {
  const empty = { ...EMPTY_FRAME, nodeCount: 0, edgeCount: 0 };
  const scene: Scene = sceneOf(empty, plainStyle(0), null);
  const moved = deferredScene(scene, { x: new Float32Array(0), y: new Float32Array(0) });
  assert.equal(moved.bounds, null);
  assert.equal(moved.grid.items.length, 0);
});

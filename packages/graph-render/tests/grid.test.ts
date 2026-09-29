import assert from "node:assert/strict";
import { test } from "node:test";

import type { Frame } from "../src/frame.ts";
import { gridOf } from "../src/grid.ts";
import { pickIn, sceneOf } from "../src/scene.ts";
import { styleFrom } from "../src/style.ts";
import { lineFrame, mulberry32, randomFrame } from "./support.ts";

function weighted(nodeCount: number, seed: number): ReturnType<typeof styleFrom> {
  const next = mulberry32(seed);
  return styleFrom({
    labels: [], weights: Float32Array.from({ length: nodeCount }, () => next()),
    colours: new Uint16Array(nodeCount), palette: ["a"],
  });
}

test("the grid agrees with a scan of every node, over 2000 random points", () => {
  const frame = randomFrame(500, 0, 11);
  const scene = sceneOf(frame, weighted(500, 12), null);
  const next = mulberry32(13);
  for (let i = 0; i < 2000; i += 1) {
    const x = next() * 1100 - 50;
    const y = next() * 1100 - 50;
    let expected = -1;
    let best = 4;
    for (let node = 0; node < frame.nodeCount; node += 1) {
      const d = Math.hypot(x - (frame.x[node] ?? 0), y - (frame.y[node] ?? 0)) - (scene.extent[node] ?? 0);
      if (d < best || (d === best && node > expected)) {
        expected = node;
        best = d;
      }
    }
    assert.equal(pickIn(scene, { x, y, tolerance: 4, floor: 0 }), expected, `point ${i}`);
  }
});

test("a point far from every node picks nothing", () => {
  const scene = sceneOf(lineFrame({ x: [0, 100], y: [0, 100] }), weighted(2, 1), null);
  assert.equal(pickIn(scene, { x: 50, y: 50, tolerance: 4, floor: 0 }), -1);
  assert.equal(pickIn(scene, { x: -9000, y: 9000, tolerance: 4, floor: 0 }), -1);
});

test("a hidden node is not picked", () => {
  const frame = lineFrame({ x: [0, 100], y: [0, 0] });
  const style = styleFrom({
    labels: [], weights: new Float32Array(2), colours: new Uint16Array(2), palette: ["a"],
    hidden: Uint8Array.from([1, 0]),
  });
  assert.equal(pickIn(sceneOf(frame, style, null), { x: 0, y: 0, tolerance: 4, floor: 0 }), -1);
});

test("a box is picked anywhere inside it", () => {
  const frame: Frame = {
    ...lineFrame({ x: [0, 300], y: [0, 0] }), nodeKind: "Box",
    w: Float32Array.from([200, 20]), h: Float32Array.from([40, 20]),
  };
  const scene = sceneOf(frame, weighted(2, 1), null);
  assert.equal(pickIn(scene, { x: 95, y: 18, tolerance: 0, floor: 0 }), 0);
  assert.equal(pickIn(scene, { x: 95, y: 30, tolerance: 0, floor: 0 }), -1);
});

test("a collinear frame does not ask for millions of cells", () => {
  const x = Array.from({ length: 5000 }, (_, i) => i * 1000);
  const grid = gridOf({ x: Float32Array.from(x), y: new Float32Array(5000) }, { minX: 0, minY: 0, maxX: 4999000, maxY: 0 });
  assert.ok(grid.columns * grid.rows <= 1 << 20);
  assert.equal(grid.items.length, 5000);
});

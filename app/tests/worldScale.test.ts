// Motor units → studio world. The regression this pins: a unit-spaced grid drawn
// raw overlapped every node and fit-to-view clamped at MAX_ZOOM, so the panel
// showed a pile of labels instead of a graph.

import assert from "node:assert/strict";
import { test } from "node:test";

import type { DrawList, NodeDraw } from "../src/core/drawList.ts";
import { POINT_RADIUS } from "../src/core/drawList.ts";
import { TARGET_SPACING, scaleDrawList, toStudioWorld, typicalSpacing, worldScaleFor } from "../src/core/worldScale.ts";

function grid(side: number, step: number): NodeDraw[] {
  const nodes: NodeDraw[] = [];
  for (let row = 0; row < side; row += 1) {
    for (let col = 0; col < side; col += 1) {
      nodes.push({ index: nodes.length, x: col * step, y: row * step, w: 0, h: 0, r: POINT_RADIUS });
    }
  }
  return nodes;
}

function list(nodes: NodeDraw[], over: Partial<DrawList> = {}): DrawList {
  return { nodeKind: "Point", edgeKind: "Line", nodes, edges: [], ...over };
}

test("a unit grid is scaled so its spacing lands near TARGET_SPACING", () => {
  const scaled = toStudioWorld(list(grid(11, 1)));
  const step = scaled.nodes[1].x - scaled.nodes[0].x;
  assert.ok(step > TARGET_SPACING * 0.8 && step < TARGET_SPACING * 1.2, `step ${step}`);
});

test("the factor is independent of the motor's unit", () => {
  assert.ok(Math.abs(worldScaleFor(grid(5, 1)) - 1000 * worldScaleFor(grid(5, 1000))) < 1e-9);
});

test("collinear nodes fall back to extent over gaps", () => {
  const line = [0, 1, 2, 3].map((x, index) => ({ index, x, y: 0, w: 0, h: 0, r: POINT_RADIUS }));
  assert.equal(typicalSpacing(line), 1);
});

test("a single node or a zero-spread run is left unscaled", () => {
  assert.equal(worldScaleFor(grid(1, 1)), 1);
  const stacked = [0, 1].map((index) => ({ index, x: 3, y: 3, w: 0, h: 0, r: POINT_RADIUS }));
  assert.equal(worldScaleFor(stacked), 1);
});

test("scaling keeps Point radius fixed but scales Circle/Box sizes and edge points", () => {
  const box = { index: 0, x: 1, y: 2, w: 4, h: 6, r: 2 };
  const scaledBox = scaleDrawList(list([box], { nodeKind: "Box" }), 10).nodes[0];
  assert.deepEqual({ ...scaledBox }, { index: 0, x: 10, y: 20, w: 40, h: 60, r: 20 });
  const point = scaleDrawList(list(grid(1, 1)), 10).nodes[0];
  assert.equal(point.r, POINT_RADIUS);
  const edge = { index: 0, source: 0, target: 0, pts: new Float32Array([1, 2, 3, 4]), degree: 0 };
  const scaledEdge = scaleDrawList(list([box], { edges: [edge] }), 2).edges[0];
  assert.deepEqual([...scaledEdge.pts], [2, 4, 6, 8]);
  assert.deepEqual([...edge.pts], [1, 2, 3, 4]);
});

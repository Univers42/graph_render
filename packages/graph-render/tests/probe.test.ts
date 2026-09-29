/** The view's own account of what it draws: opacity per node and edge, and the labels. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { edgeOpacity, labelledNodes, nodeOpacity } from "../src/canvas2d/probe.ts";
import { DARK_THEME } from "../src/theme.ts";

// 0-1, 1-2, 2-3: focusing node 1 lights 0, 1, 2 and the edges 0-1 and 1-2.
const source = Uint32Array.from([0, 1, 2]);
const target = Uint32Array.from([1, 2, 3]);

function reading(hovered: number, dimStart: number) {
  const lit = new Uint8Array(4);
  if (hovered >= 0) {
    lit[hovered] = 1;
    for (const node of [0, 1, 2]) if (hovered === 1) lit[node] = 1;
  }
  return {
    hovered, selected: -1, dimStart, theme: DARK_THEME, lit,
    scene: { frame: { source, target } },
    plan: { count: 2, node: Uint32Array.from([1, 0, 9]) },
  };
}

test("with no focus everything is at opacity 1", () => {
  const state = reading(-1, -1);
  assert.equal(nodeOpacity(state, 3, 500), 1);
  assert.equal(edgeOpacity(state, 2, 500), 1);
});

test("after the fade the focus and its edges are at 1 and the rest at 0.12", () => {
  const state = reading(1, 1000);
  const now = 1000 + 120;
  assert.deepEqual([0, 1, 2, 3].map((n) => nodeOpacity(state, n, now)), [1, 1, 1, 0.12]);
  assert.deepEqual([0, 1, 2].map((e) => edgeOpacity(state, e, now)), [1, 1, 0.12]);
});

test("halfway through the fade the dimmed nodes are between the two", () => {
  const state = reading(1, 1000);
  assert.ok(Math.abs(nodeOpacity(state, 3, 1060) - 0.56) < 1e-12);
});

test("the labelled nodes are the first `count` of the plan", () => {
  assert.deepEqual(labelledNodes(reading(-1, -1)), [1, 0]);
});

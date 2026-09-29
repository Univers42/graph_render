import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf, bfsMask, type WalkKinds } from "../src/adjacency.ts";

type Pairs = readonly (readonly [number, number])[];

const BOTH: WalkKinds = { incoming: true, outgoing: true, neighbours: false };
const OUT: WalkKinds = { incoming: false, outgoing: true, neighbours: false };
const IN: WalkKinds = { incoming: true, outgoing: false, neighbours: false };
const ANY: WalkKinds = { incoming: false, outgoing: false, neighbours: false };

function maskOf(graph: { readonly count: number; readonly pairs: Pairs }, root: number, depth: number, kinds: WalkKinds): number[] {
  const { count, pairs } = graph;
  const adjacency = adjacencyOf(count, {
    source: Uint32Array.from(pairs, (pair) => pair[0]),
    target: Uint32Array.from(pairs, (pair) => pair[1]),
  });
  return [...bfsMask(adjacency, root, depth, kinds)];
}

const TRIANGLE: Pairs = [[0, 1], [1, 2], [2, 0]];
const LINE: Pairs = [[0, 1], [1, 2], [2, 3]];

test("triangle, depth 1, both directions", () => {
  assert.deepEqual(maskOf({ count: 3, pairs: TRIANGLE }, 0, 1, BOTH), [1, 1, 1]);
});

test("triangle, depth 1, outgoing only", () => {
  assert.deepEqual(maskOf({ count: 3, pairs: TRIANGLE }, 0, 1, OUT), [1, 1, 0]);
});

test("triangle, depth 1, incoming only", () => {
  assert.deepEqual(maskOf({ count: 3, pairs: TRIANGLE }, 0, 1, IN), [1, 0, 1]);
});

test("triangle, depth 2, outgoing only reaches the far node", () => {
  assert.deepEqual(maskOf({ count: 3, pairs: TRIANGLE }, 0, 2, OUT), [1, 1, 1]);
});

test("line, outgoing: depth 2 stops at node 2, depth 3 reaches node 3", () => {
  assert.deepEqual(maskOf({ count: 4, pairs: LINE }, 0, 2, OUT), [1, 1, 1, 0]);
  assert.deepEqual(maskOf({ count: 4, pairs: LINE }, 0, 3, OUT), [1, 1, 1, 1]);
});

test("line, from the middle, depth 1: no flag means both ways, neighbours means both ways", () => {
  assert.deepEqual(maskOf({ count: 4, pairs: LINE }, 1, 1, ANY), [1, 1, 1, 0]);
  assert.deepEqual(maskOf({ count: 4, pairs: LINE }, 1, 1, { ...OUT, neighbours: true }), [1, 1, 1, 0]);
});

test("line, from the middle, outgoing only never walks back", () => {
  assert.deepEqual(maskOf({ count: 4, pairs: LINE }, 2, 5, OUT), [0, 0, 1, 1]);
});

test("same input, same mask", () => {
  const pairs: Pairs = [[0, 1], [0, 2], [1, 3], [2, 3], [3, 4]];
  assert.deepEqual(maskOf({ count: 5, pairs }, 0, 2, BOTH), maskOf({ count: 5, pairs: pairs }, 0, 2, BOTH));
  assert.deepEqual(maskOf({ count: 5, pairs }, 0, 2, BOTH), [1, 1, 1, 1, 0]);
});

test("parallel edges and a self-loop add no node twice", () => {
  assert.deepEqual(maskOf({ count: 2, pairs: [[0, 1], [0, 1]] }, 0, 1, OUT), [1, 1]);
  assert.deepEqual(maskOf({ count: 2, pairs: [[0, 0], [0, 1]] }, 0, 1, OUT), [1, 1]);
});

test("a root or depth out of range gives an empty mask", () => {
  assert.deepEqual(maskOf({ count: 3, pairs: TRIANGLE }, 3, 1, BOTH), [0, 0, 0]);
  assert.deepEqual(maskOf({ count: 3, pairs: TRIANGLE }, 0, 0, BOTH), [0, 0, 0]);
  assert.deepEqual(maskOf({ count: 3, pairs: TRIANGLE }, 0, 6, BOTH), [0, 0, 0]);
});

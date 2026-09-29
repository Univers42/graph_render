import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf, degreeOf, markNeighbourhood } from "../src/adjacency.ts";
import { randomFrame } from "./support.ts";

type Pairs = readonly (readonly [number, number])[];

function neighbours(nodeCount: number, pairs: Pairs, node: number): number[] {
  const ends = { source: Uint32Array.from(pairs, (p) => p[0]), target: Uint32Array.from(pairs, (p) => p[1]) };
  const adjacency = adjacencyOf(nodeCount, ends);
  return [...adjacency.neighbour.subarray(adjacency.start[node], adjacency.start[node + 1])].sort((a, b) => a - b);
}

test("an edge lists each end at the other", () => {
  const pairs: Pairs = [[0, 1], [1, 2]];
  assert.deepEqual(neighbours(3, pairs, 1), [0, 2]);
  assert.deepEqual(neighbours(3, pairs, 0), [1]);
});

test("a self-loop lists its node once and a parallel edge lists twice", () => {
  assert.deepEqual(neighbours(2, [[0, 0], [0, 1], [1, 0]], 0), [0, 1, 1]);
});

test("a graph with no edges has empty rows", () => {
  const adjacency = adjacencyOf(3, { source: new Uint32Array(0), target: new Uint32Array(0) });
  assert.deepEqual([...adjacency.start], [0, 0, 0, 0]);
});

test("every entry names the edge behind it", () => {
  const frame = randomFrame(60, 400, 7);
  const adjacency = adjacencyOf(frame.nodeCount, frame);
  let entries = 0;
  for (let node = 0; node < frame.nodeCount; node += 1) {
    for (let at = adjacency.start[node] ?? 0; at < (adjacency.start[node + 1] ?? 0); at += 1) {
      const edge = adjacency.edge[at] ?? 0;
      const ends = [frame.source[edge], frame.target[edge]];
      assert.ok(ends.includes(node) && ends.includes(adjacency.neighbour[at]));
      entries += 1;
    }
    assert.equal(degreeOf(adjacency, node), (adjacency.start[node + 1] ?? 0) - (adjacency.start[node] ?? 0));
  }
  const loops = frame.source.reduce((sum, s, e) => sum + (s === frame.target[e] ? 1 : 0), 0);
  assert.equal(entries, 2 * frame.edgeCount - loops);
});

test("the lit mask is the node and its neighbours, and -1 clears it", () => {
  const adjacency = adjacencyOf(4, { source: Uint32Array.from([0, 1]), target: Uint32Array.from([1, 2]) });
  const mask = new Uint8Array(4).fill(1);
  markNeighbourhood(adjacency, 1, mask);
  assert.deepEqual([...mask], [1, 1, 1, 0]);
  markNeighbourhood(adjacency, -1, mask);
  assert.deepEqual([...mask], [0, 0, 0, 0]);
});

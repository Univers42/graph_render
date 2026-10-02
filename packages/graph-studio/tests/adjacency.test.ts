import assert from "node:assert/strict";
import { test } from "node:test";

import { adjacencyOf, buildAdjacency, neighboursOf } from "../src/studio/adjacency.ts";
import type { Ends } from "../src/source/meta.ts";

// 0 self-loops to itself; 1 is linked to 0 twice and 0 back to 1 once, so a duplicate and a
// reversed duplicate; 2 and 3 hang off several nodes; 4 only from 3; 5 has no edge at all.
const ENDS: Ends = {
  source: Uint32Array.of(0, 0, 0, 1, 0, 1, 2, 3, 3, 0, 0),
  target: Uint32Array.of(0, 1, 1, 0, 2, 2, 3, 4, 3, 0, 1),
};
const NODES = 6;

/** The scan this replaces, kept here so the order it produced is the oracle. */
function scanned(ends: Ends, node: number): readonly number[] {
  const found = new Set<number>();
  for (let e = 0; e < ends.source.length; e += 1) {
    const s = ends.source[e] ?? 0;
    const t = ends.target[e] ?? 0;
    if (s === node && t !== node) found.add(t);
    if (t === node && s !== node) found.add(s);
  }
  return [...found];
}

/** The same scan with the self-loop guard left out; the control for the guard. */
function scanningSelfLoops(ends: Ends, node: number): readonly number[] {
  const found = new Set<number>();
  for (let e = 0; e < ends.source.length; e += 1) {
    const s = ends.source[e] ?? 0;
    const t = ends.target[e] ?? 0;
    if (s === node) found.add(t);
    if (t === node) found.add(s);
  }
  return [...found];
}

test("every node gets the order the old scan gave it", () => {
  const adjacency = adjacencyOf(ENDS);
  for (let node = 0; node < NODES; node += 1) {
    assert.deepEqual(adjacency.at(node), scanned(ENDS, node), `node ${node}`);
  }
});

test("the order is first-encounter, duplicates once, self-loops nowhere", () => {
  const adjacency = adjacencyOf(ENDS);
  assert.deepEqual(adjacency.at(0), [1, 2]);
  assert.deepEqual(adjacency.at(1), [0, 2]);
  assert.deepEqual(adjacency.at(2), [0, 1, 3]);
  assert.deepEqual(adjacency.at(3), [2, 4]);
  assert.deepEqual(adjacency.at(4), [3]);
  assert.deepEqual(adjacency.at(5), []);
});

// A self-loop is only ever seen by keeping it, and the build drops it in both directions.
test("the self-loop guard is what the order rests on", () => {
  const adjacency = adjacencyOf(ENDS);
  assert.ok(!adjacency.at(0).includes(0));
  assert.ok(!adjacency.at(3).includes(3));
  assert.notDeepEqual(scanningSelfLoops(ENDS, 0), scanned(ENDS, 0));
  assert.notDeepEqual(scanningSelfLoops(ENDS, 3), scanned(ENDS, 3));
});

test("the adjacency is built once per Ends and rebuilt for a new one", () => {
  assert.equal(adjacencyOf(ENDS), adjacencyOf(ENDS));
  assert.equal(neighboursOf(ENDS, 2), neighboursOf(ENDS, 2));
  assert.equal(neighboursOf(ENDS, 5), neighboursOf(ENDS, 5));
  const twin: Ends = { source: ENDS.source.slice(), target: ENDS.target.slice() };
  assert.notEqual(adjacencyOf(twin), adjacencyOf(ENDS));
  assert.deepEqual(adjacencyOf(twin).at(2), adjacencyOf(ENDS).at(2));
});

test("a builder call bypasses the cache and agrees with it", () => {
  const fresh = buildAdjacency(ENDS);
  assert.notEqual(fresh, adjacencyOf(ENDS));
  assert.deepEqual(fresh.at(1), adjacencyOf(ENDS).at(1));
  assert.deepEqual(fresh.at(99), []);
});

test("an Ends with no edges answers empty for every node", () => {
  const bare: Ends = { source: new Uint32Array(0), target: new Uint32Array(0) };
  const adjacency = adjacencyOf(bare);
  for (let node = 0; node < NODES; node += 1) assert.deepEqual(adjacency.at(node), []);
  assert.deepEqual(neighboursOf(bare, 3), []);
});
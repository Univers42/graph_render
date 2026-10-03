// The d3-force arm's input and output contracts, pinned without a node process, a d3
// install or a wall clock (`harness/stress-d3/validate.mjs`, `d3-version.mjs` and
// `harness/stress-d3/seed.mjs` are all pure). Each test here is the negative case for a
// review finding: the refusal it asserts is one the arm previously did not make, so an
// arm that lost the check fails this file rather than the gate.

import { test } from "node:test";
import assert from "node:assert/strict";

import { PINNED_D3_VERSION, versionRefusal } from "./d3-version.mjs";
import { caseRefusal, positionsRefusal } from "./stress-d3/validate.mjs";
import { goldenSpiral, seedMovesWithAngle } from "./stress-d3/seed.mjs";

test("m85: an edge naming a node outside 0..n-1 is refused with its case number", () => {
  // Before the fix this reached d3 and came back as an unnamed `node not found: 7`.
  assert.equal(caseRefusal(3, { seed: 1, n: 2, edges: [[0, 7]] }), "case 3: edge 0 names a node outside 0..1");
  assert.equal(caseRefusal(3, { seed: 1, n: 2, edges: [[-1, 0]] }), "case 3: edge 0 names a node outside 0..1");
});

test("m85: an edge that is not a pair of integers is refused", () => {
  assert.match(caseRefusal(0, { seed: 1, n: 2, edges: [[0, 1.5]] }), /not a pair of integers/);
  assert.match(caseRefusal(0, { seed: 1, n: 2, edges: [[0]] }), /not a \[lo,hi\] pair/);
  assert.match(caseRefusal(0, { seed: 1, n: 2, edges: [[0, 1, 2]] }), /not a \[lo,hi\] pair/);
});

test("m85: a case without an integer seed is refused, not run and silently dropped", () => {
  // Before the fix this ran to completion and wrote a line with no `seed` member at all.
  assert.equal(caseRefusal(0, { n: 2, edges: [[0, 1]] }), "case 0: no integer seed");
});

test("m85: a well-formed case is not refused", () => {
  assert.equal(caseRefusal(0, { seed: 1, n: 2, edges: [[0, 1]] }), null);
  assert.equal(caseRefusal(0, { seed: 1, n: 0, edges: [] }), null);
  assert.equal(caseRefusal(0, { seed: 7, n: 3, edges: [[2, 2]] }), null);
});

test("m85: a line that is not an object, or has no node count, is refused", () => {
  assert.match(caseRefusal(1, null), /not a JSON object/);
  assert.match(caseRefusal(1, [1, 2]), /not a JSON object/);
  assert.match(caseRefusal(1, { seed: 1, edges: [] }), /no node count/);
  assert.match(caseRefusal(1, { seed: 1, n: -1, edges: [] }), /no node count/);
  assert.match(caseRefusal(1, { seed: 1, n: 2, edges: "0,1" }), /edges is not an array/);
});

test("m86: a non-finite position is refused, naming the case and the node", () => {
  // Before the fix `JSON.stringify` wrote the NaN as `null` and the arm exited 0.
  assert.equal(
    positionsRefusal(2, [1, Number.NaN], [0, 0]),
    "case 2: node 1 reached a non-finite position (NaN, 0)",
  );
  assert.equal(positionsRefusal(0, [1], [Number.POSITIVE_INFINITY]), "case 0: node 0 reached a non-finite position (1, Infinity)");
});

test("m86: mismatched coordinate counts and finite positions are not refused", () => {
  assert.match(positionsRefusal(0, [1], [1, 2]), /1 x coordinates against 2 y/);
  assert.equal(positionsRefusal(0, [], []), null);
  assert.equal(positionsRefusal(0, [1.5, -2.5], [0, 0]), null);
});

test("m83: only the pinned d3-force version is accepted", () => {
  assert.equal(versionRefusal(PINNED_D3_VERSION), null);
  assert.match(versionRefusal("3.0.1"), /resolved d3-force is 3\.0\.1, not the pinned 3\.0\.0/);
  assert.match(versionRefusal(null), /carries no readable version/);
});

test("m87: the golden-spiral seed is load-bearing — one ulp of angle moves it", () => {
  // This is the witness for the hazard `harness/stress-d3/seed.mjs` records: if the
  // spiral were insensitive to a one-ulp change, the libm-vs-V8 gap it describes could
  // not matter, and the note would be wrong.
  assert.equal(seedMovesWithAngle(8), true);
  const { x, y } = goldenSpiral(4);
  assert.equal(x.length, 4);
  assert.equal(y.length, 4);
  assert.ok([...x, ...y].every(Number.isFinite));
  // A self-contained golden spiral: node 0 sits on the +x axis at the seed radius.
  assert.equal(x[0], 12);
  assert.equal(y[0], 0);
});
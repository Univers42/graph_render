import assert from "node:assert/strict";
import { test } from "node:test";

import { MetaMismatch, UNGROUPED, metaOf } from "../src/source/meta.ts";
import { node } from "./support.ts";

const NODES = [
  node("a", { group: "g1", weight: 0.25, kind: "note" }),
  node("b", { label: "Bee" }),
  node("c", { group: "g1", weight: 1 }),
];

test("the columns follow the snapshot's order, not the document's", () => {
  const meta = metaOf(NODES, ["c", "a", "b"], { source: Uint32Array.of(0, 0), target: Uint32Array.of(1, 2) });
  assert.deepEqual(meta.ids, ["c", "a", "b"]);
  assert.deepEqual(meta.labels, ["C", "A", "Bee"]);
  assert.deepEqual(meta.kinds, ["record", "note", "record"]);
  assert.deepEqual(meta.groups, ["g1", UNGROUPED]);
  assert.deepEqual([...meta.group], [0, 0, 1]);
  assert.deepEqual([...meta.weight], [1, 0.25, 0.5]);
  assert.deepEqual([...meta.degree], [2, 1, 1]);
  assert.equal(meta.maxDegree, 2);
});

test("a self-loop counts once and a parallel edge counts each time", () => {
  const meta = metaOf(NODES, ["a", "b", "c"], { source: Uint32Array.of(0, 1, 1), target: Uint32Array.of(0, 2, 2) });
  assert.deepEqual([...meta.degree], [1, 2, 2]);
});

test("an id the document does not hold is refused by name", () => {
  assert.throws(
    () => metaOf(NODES, ["a", "b", "z"], { source: new Uint32Array(0), target: new Uint32Array(0) }),
    (error: unknown) => error instanceof MetaMismatch && /node "z"/.test(error.message),
  );
});

test("a snapshot with fewer nodes than the document is refused", () => {
  assert.throws(() => metaOf(NODES, ["a", "b"], { source: new Uint32Array(0), target: new Uint32Array(0) }), MetaMismatch);
});

test("no nodes is an empty meta, not a refusal", () => {
  const meta = metaOf([], [], { source: new Uint32Array(0), target: new Uint32Array(0) });
  assert.equal(meta.nodeCount, 0);
  assert.deepEqual(meta.groups, []);
  assert.equal(meta.maxDegree, 0);
});

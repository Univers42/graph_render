import assert from "node:assert/strict";
import { test } from "node:test";

import { MetaMismatch, UNGROUPED, metaOf } from "../src/source/meta.ts";
import { node } from "./support.ts";

const NODES = [
  node("a", { group: "g1", weight: 0.25, kind: "note", tags: ["Zed", "alpha"], path: "notes/a.md", database_id: "db-1" }),
  node("b", { label: "Bee" }),
  node("c", { group: "g1", weight: 1, tags: ["beta"], path: "notes/c.md", database_id: "db-1" }),
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

// The three columns the sources did not carry until now, in the snapshot's order like
// every other column, and never in the document's: `inOrder` joins by id.
test("tags, dbs and paths follow the snapshot's order, not the document's", () => {
  const meta = metaOf(NODES, ["c", "a", "b"], { source: Uint32Array.of(0, 0), target: Uint32Array.of(1, 2) });
  assert.deepEqual(meta.tags, [["beta"], ["Zed", "alpha"], []]);
  assert.deepEqual(meta.dbs, ["db-1", "db-1", ""]);
  assert.deepEqual(meta.paths, ["notes/c.md", "notes/a.md", ""]);
});

test("a node's tags keep their document order, two or none", () => {
  const meta = metaOf(NODES, ["a", "b", "c"], { source: new Uint32Array(0), target: new Uint32Array(0) });
  assert.deepEqual(meta.tags[0], ["Zed", "alpha"]);
  assert.deepEqual(meta.tags[1], []);
  assert.equal(meta.tags.length, meta.nodeCount);
  assert.deepEqual(meta.dbs, ["db-1", "", "db-1"]);
  assert.deepEqual(meta.paths, ["notes/a.md", "", "notes/c.md"]);
});

// `tags` and `path` are optional on the way in, so a column read must not have to
// test for undefined: an old document still reads as "no tags, no path".
test("a node without the optional members reads as no tags and no path", () => {
  const legacy = [node("x"), node("y")];
  const meta = metaOf(legacy, ["x", "y"], { source: Uint32Array.of(0), target: Uint32Array.of(1) });
  assert.deepEqual(meta.tags, [[], []]);
  assert.deepEqual(meta.dbs, ["", ""]);
  assert.deepEqual(meta.paths, ["", ""]);
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
  assert.deepEqual(meta.tags, []);
  assert.deepEqual(meta.dbs, []);
  assert.deepEqual(meta.paths, []);
  assert.equal(meta.maxDegree, 0);
});

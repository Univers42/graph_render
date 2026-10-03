// The wasm arm's ingest-document contract, pinned without a wasm binary, a node process
// or a wall clock (`harness/wasm-tick-bench/{contract,document}.mjs` are pure). Each test
// here is the negative case for a review finding: the refusal it asserts is one the arm
// previously did not make, so an arm that lost the check fails this file rather than the
// bench, which is a measurement and must not carry the job.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

import { memberRefusal, membersFromContract, provisionalIngestExample, requiredMembers } from "./wasm-tick-bench/contract.mjs";
import { documentRefusal, edgeOf, ingestOf, nodeOf } from "./wasm-tick-bench/document.mjs";

const ROOT = resolve(import.meta.dirname, "..");

test("m82: the member lists come from the committed contract, not from beside the writer", () => {
  const members = membersFromContract(ROOT);
  assert.deepEqual(members.node, [
    "id", "kind", "database_id", "source", "label", "group", "weight", "version", "has_note", "icon",
  ]);
  // The contract's example omits `child_first` (optional in version 1); the arm states it
  // explicitly, so the edge list is the contract's members plus that one.
  assert.deepEqual(members.edge, [
    "id", "source", "target", "kind", "label", "strength", "directed", "record_id", "child_first",
  ]);
});

test("m82: the lists track the file, so a contract edit is not silently ignored", () => {
  // Before the fix the lists were a hand copy: editing either side together still passed.
  const markdown = readFileSync(resolve(ROOT, "docs", "contract", "wasm-abi.md"), "utf8");
  const example = provisionalIngestExample(markdown);
  example.nodes[0] = { ...example.nodes[0], tone: null };
  assert.deepEqual(requiredMembers(example).node.slice(-1), ["tone"]);
});

test("m82: a document missing a required member is refused, naming it", () => {
  // Before the fix this was compared against the writer's own copy, so dropping a member
  // from the writer changed the expectation with it and nothing went red.
  const members = membersFromContract(ROOT);
  const document = { version: 1, nodes: [nodeOf({ id: "a" })], edges: [edgeOf({ id: "e" })] };
  assert.equal(documentRefusal(document, members), null);
  const { has_note, ...withoutNote } = document.nodes[0];
  void has_note;
  const refusal = documentRefusal({ ...document, nodes: [withoutNote] }, members);
  assert.match(refusal, /node record \(0\) does not carry exactly the contract's members: missing has_note/);
});

test("m82: a document with a stray member is refused, naming it", () => {
  const members = membersFromContract(ROOT);
  const node = { ...nodeOf({ id: "a" }), hasNote: false };
  assert.match(
    documentRefusal({ version: 1, nodes: [node], edges: [edgeOf({ id: "e" })] }, members),
    /unexpected hasNote/,
  );
});

test("m82: a document of the wrong version, or with no node or no edge, is refused", () => {
  const members = membersFromContract(ROOT);
  const node = nodeOf({ id: "a" });
  const edge = edgeOf({ id: "e" });
  assert.equal(documentRefusal({ version: 2, nodes: [node], edges: [edge] }, members), "the ingest document's version is 2, not 1");
  assert.equal(documentRefusal({ version: 1, nodes: [], edges: [edge] }, members), "the ingest document holds no node");
  assert.equal(documentRefusal({ version: 1, nodes: [node], edges: [] }, members), "the ingest document holds no edge");
});

test("m82: the member comparison is order-insensitive and names both directions", () => {
  assert.equal(memberRefusal("a record", ["a", "b"], ["b", "a"]), null);
  assert.equal(memberRefusal("a record", ["a"], ["a", "b"]), "a record does not carry exactly the contract's members: missing b");
  assert.equal(memberRefusal("a record", ["a", "b", "c"], ["a", "b"]), "a record does not carry exactly the contract's members: unexpected c");
});

test("m82: `ingestOf` writes version 1, nodes then edges, and nothing else", () => {
  const document = ingestOf({ nodes: [{ id: "a" }], edges: [{ id: "e" }] });
  assert.deepEqual(Object.keys(document), ["version", "nodes", "edges"]);
  assert.equal(document.version, 1);
  assert.equal(document.nodes.length, 1);
  assert.equal(document.edges.length, 1);
});
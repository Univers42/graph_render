// The normaliser read one document at a time, and the hot path was rewritten for a 62 MB
// document (85 928 nodes, 107 694 edges). These pin what the rewrite must not change: the wire
// text byte for byte, the members in the contract's own order, and every note and refusal —
// in the order they happen. A faster normaliser that drops an annotation note, reorders a
// member or stops counting the defaults is not the same normaliser.

import assert from "node:assert/strict";
import { test } from "node:test";

import { IngestRefusal, normaliseIngest } from "../src/source/ingest.ts";

/** The contract's ten, in the order the wire text carries them (`ingest.ts`, `wireNode`). */
const CONTRACT_MEMBERS = [
  "id", "kind", "database_id", "source", "label", "group",
  "weight", "version", "has_note", "icon",
];

const FULL = JSON.stringify({
  version: 1,
  nodes: [{
    id: "a", kind: "note", database_id: "db", source: "git", label: "A", group: "G",
    weight: 0.25, version: 7, has_note: true, icon: "star", tags: ["Zed", "alpha"], path: "n/a.md",
  }],
  edges: [{
    id: "e", source: "a", target: "a", kind: "tag", label: "L", strength: 1,
    directed: true, record_id: "db", child_first: true,
  }],
});

test("a full document passes through with no notes and the ten members in contract order", () => {
  const result = normaliseIngest(FULL, "full.json");
  assert.deepEqual(result.notes, []);
  assert.equal(result.json, '{"version":1,"nodes":[' +
    '{"id":"a","kind":"note","database_id":"db","source":"git","label":"A","group":"G",' +
    '"weight":0.25,"version":7,"has_note":true,"icon":"star"}],"edges":[' +
    '{"id":"e","source":"a","target":"a","kind":"tag","label":"L","strength":1,' +
    '"directed":true,"record_id":"db","child_first":true}]}');
  // `tags` and `path` ride in `doc`, never on the wire: `gm_build` refuses an unknown member.
  assert.deepEqual(result.doc.nodes[0]?.tags, ["Zed", "alpha"]);
  assert.equal(result.doc.nodes[0].path, "n/a.md");
});

// The one note a mapped `type` produces names the spelling as the document wrote it, and it is
// produced whether the spelling matched as it stands or only after lowercasing. A silent mapping
// is a lie about what was read: the note is the only place the user learns the studio guessed.
test("a `type` that only matches lowercased still says so, naming the spelling as written", () => {
  const { doc, notes } = normaliseIngest(JSON.stringify({
    nodes: [{ id: "a" }, { id: "b" }],
    edges: [{ id: "e", source: "a", target: "b", type: "Parent" }],
  }), "case.json");
  assert.equal(doc.edges[0]?.kind, "hierarchy");
  assert.ok(notes.includes('mapped edge `type` "Parent" to kind "hierarchy"'),
    `the note names the spelling as written; got ${JSON.stringify(notes)}`);
});

test("every note of a ragged document, in the order they happen", () => {
  const result = normaliseIngest(JSON.stringify({
    about: "a tree",
    nodes: [
      { id: "a", role: "root", tags: ["keep", 7] },
      "b",
      { id: "c", kind: "note", weight: "heavy", has_note: null, icon: 3, path: 42 },
    ],
    edges: [
      { id: "e", source: "a", target: "b", type: "child_of", colour: "red" },
      { id: "f", source: "b", target: "c", type: "mentions" },
    ],
  }), "ragged.json");
  assert.deepEqual(result.notes, [
    "dropped annotation `about`",
    "no `version` member: assumed 1",
    "dropped annotation `nodes[0].role`",
    "defaulted 9 member(s) on node \"a\"",
    "dropped `nodes[0].tags`: not a list of strings",
    'node "b" was given as a bare id',
    "defaulted 9 member(s) on node \"b\"",
    "defaulted 5 member(s) on node \"c\"",
    "dropped annotation `edges[0].colour`",
    "mapped edge `type` \"child_of\" to kind \"hierarchy\"",
    "defaulted 6 member(s) on edge \"e\"",
    "defaulted 6 member(s) on edge \"f\"",
  ]);
  assert.equal(result.doc.edges[0]?.kind, "hierarchy");
  assert.equal(result.doc.edges[1]?.kind, "relation");
  assert.deepEqual(result.doc.nodes.map((node) => [node.id, node.kind, node.has_note, node.icon, node.path]),
    [["a", "record", false, null, ""], ["b", "record", false, null, ""], ["c", "note", true, null, ""]]);
});

test("a duplicate id, a dangling end and a bad kind are refused, each naming its index", () => {
  const nodes = '[{"id":"a"},{"id":"a"}]';
  assert.throws(() => normaliseIngest(`{"nodes":${nodes},"edges":[]}`, "d.json"), /d.json: duplicate node id "a"/);
  assert.throws(
    () => normaliseIngest('{"nodes":[{"id":"a"}],"edges":[{"id":"e","source":"a","target":"z"}]}', "g.json"),
    /g.json: edge "e" targets "z", which is not a node/,
  );
  assert.throws(
    () => normaliseIngest('{"nodes":[{"id":"a"}],"edges":[{"id":"e","source":"z","target":"a"}]}', "s.json"),
    /s.json: edge "e" sources "z", which is not a node/,
  );
  assert.throws(() => normaliseIngest('{"nodes":[{"id":"a","kind":"Record"}],"edges":[]}', "k.json"),
    IngestRefusal);
  assert.throws(() => normaliseIngest('{"nodes":[{"id":"a"}],"edges":[{"id":"e","source":"a","target":"a","kind":"link"}]}', "k.json"),
    /k.json: edges\[0\].kind "link" is not an edge kind/);
});

// The hot path, at a size where a per-node allocation shows: the wire text of 20 000 nodes is
// built from the ten members in the contract's order, and no member is dropped or renamed.
test("20 000 nodes normalise to the same wire text as one node at a time", () => {
  const many = {
    version: 1,
    nodes: Array.from({ length: 20_000 }, (_, i) => ({
      id: `n-${i}`, kind: "record", database_id: "db", source: "git", label: `L${i}`,
      group: `G${i % 7}`, weight: 0.5, version: i, has_note: false, icon: null,
      tags: i % 3 === 0 ? ["t"] : [],
    })),
    edges: Array.from({ length: 19_999 }, (_, i) => ({
      id: `e-${i}`, source: `n-${i}`, target: `n-${i + 1}`, kind: "relation", label: "parents",
      strength: 1, directed: true, record_id: null, child_first: false,
    })),
  };
  const once = normaliseIngest(JSON.stringify(many), "many.json");
  const byHand = many.nodes.map((node) => JSON.stringify(Object.fromEntries(
    CONTRACT_MEMBERS.map((member) => [member, member === "id" ? node.id
      : member === "kind" ? node.kind
        : member === "database_id" ? node.database_id
          : member === "source" ? node.source
            : member === "label" ? node.label
              : member === "group" ? node.group
                : member === "weight" ? node.weight
                  : member === "version" ? node.version
                    : member === "has_note" ? node.has_note : node.icon]))));
  assert.deepEqual(once.notes, []);
  assert.equal(once.doc.nodes.length, 20_000);
  assert.equal(once.doc.edges.length, 19_999);
  assert.equal(once.json, `{"version":1,"nodes":[${byHand.join(",")}],"edges":${JSON.stringify(many.edges)}}`);
});
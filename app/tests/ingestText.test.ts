// Turning whatever the user hands the studio into a document `gm_build` will
// accept. Two jobs, both pinned here: the synthetic model's exact bytes, and the
// normaliser that fills a fixture's shorthand records out to the full provisional
// ingest shape — including every refusal, which must be loud.

import assert from "node:assert/strict";
import { test } from "node:test";

import { type IngestDoc, IngestRefusal, normaliseIngest } from "../src/core/ingestText.ts";
import { syntheticIngest, syntheticRecords } from "../src/core/synthetic.ts";

// The synthetic model, in full: mulberry32 (the stream `src/core/model/synthetic.ts`
// documents, `next = ((t ^ (t >>> 14)) >>> 0) / 2^32`), then per node two draws
// (kind roll, group index) and per edge two (target index, kind roll), in that
// order. The literals below were computed from that stream, not recorded from a
// run of the implementation, so a drifted PRNG or a reordered draw fails here.
const SEED_1_EXPECTED =
  '{"version":1,"nodes":[' +
  '{"id":"n-0","kind":"note","database_id":"db-0","source":"studio","label":"Node 0","group":"Alpha","weight":0.75,"version":0,"has_note":true,"icon":null},' +
  '{"id":"n-1","kind":"record","database_id":"db-1","source":"studio","label":"Node 1","group":"Gamma","weight":0.25,"version":0,"has_note":false,"icon":null},' +
  '{"id":"n-2","kind":"database","database_id":"db-2","source":"studio","label":"Node 2","group":"Alpha","weight":1,"version":0,"has_note":false,"icon":null},' +
  '{"id":"n-3","kind":"record","database_id":"db-3","source":"studio","label":"Node 3","group":"Beta","weight":0.5,"version":0,"has_note":false,"icon":null},' +
  '{"id":"n-4","kind":"record","database_id":"db-4","source":"studio","label":"Node 4","group":"Gamma","weight":0.5,"version":0,"has_note":false,"icon":null}' +
  '],"edges":[' +
  '{"id":"e-0","source":"n-2","target":"n-0","kind":"hierarchy","label":"hierarchy","strength":0.5,"directed":true,"record_id":null,"child_first":false},' +
  '{"id":"e-1","source":"n-2","target":"n-0","kind":"relation","label":"relation","strength":0.5,"directed":false,"record_id":null,"child_first":false},' +
  '{"id":"e-2","source":"n-3","target":"n-1","kind":"hierarchy","label":"hierarchy","strength":0.5,"directed":true,"record_id":null,"child_first":false},' +
  '{"id":"e-3","source":"n-3","target":"n-0","kind":"relation","label":"relation","strength":0.5,"directed":false,"record_id":null,"child_first":false},' +
  '{"id":"e-4","source":"n-4","target":"n-2","kind":"relation","label":"relation","strength":0.5,"directed":false,"record_id":null,"child_first":false},' +
  '{"id":"e-5","source":"n-4","target":"n-2","kind":"relation","label":"relation","strength":0.5,"directed":false,"record_id":null,"child_first":false}' +
  "]}";

test("the synthetic document for seed 1, five nodes, degree 2 is exactly this", () => {
  assert.equal(syntheticIngest({ seed: 1, nodeCount: 5, degree: 2 }), SEED_1_EXPECTED);
});

test("the same spec twice is byte-identical, and a different seed is not", () => {
  const a = syntheticIngest({ seed: 7, nodeCount: 40, degree: 3 });
  assert.equal(a, syntheticIngest({ seed: 7, nodeCount: 40, degree: 3 }));
  assert.notEqual(a, syntheticIngest({ seed: 8, nodeCount: 40, degree: 3 }));
});

test("node count and edge count follow the spec exactly, with no dangling endpoint", () => {
  const spec = { seed: 3, nodeCount: 25, degree: 4 };
  const { nodes, edges } = syntheticRecords(spec);
  assert.equal(nodes.length, 25);
  assert.equal(edges.length, (25 - 4) * 4);
  const ids = new Set(nodes.map((n) => n.id));
  assert.equal(ids.size, 25);
  for (const edge of edges) {
    assert.ok(ids.has(edge.source), `${edge.id} source ${edge.source}`);
    assert.ok(ids.has(edge.target), `${edge.id} target ${edge.target}`);
  }
});

test("weight is the node's degree over the graph's maximum degree", () => {
  const { nodes, edges } = syntheticRecords({ seed: 3, nodeCount: 25, degree: 4 });
  const degree = new Map<string, number>(nodes.map((n) => [n.id, 0]));
  for (const edge of edges) {
    degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
    degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
  }
  const max = Math.max(...degree.values());
  for (const node of nodes) {
    assert.equal(node.weight, (degree.get(node.id) ?? 0) / max);
  }
});

// The kind roll's thresholds are the generator's only tuned numbers, and they are
// invisible in a five-node document (no draw lands near a boundary). Over 2000
// nodes every threshold move shows up in the census, so the census is pinned.
test("the kind census over 2000 seeded nodes is exactly this", () => {
  const { nodes } = syntheticRecords({ seed: 1, nodeCount: 2000, degree: 2 });
  const census: Record<string, number> = { record: 0, note: 0, database: 0, tag: 0 };
  for (const node of nodes) census[node.kind] += 1;
  assert.deepEqual(census, { record: 1656, note: 94, database: 135, tag: 115 });
});

test("a spec is clamped, never refused: too few nodes, zero degree, huge seed", () => {
  const clamped = syntheticRecords({ seed: 4294967295, nodeCount: 1, degree: 0 });
  assert.equal(clamped.nodes.length, 2);
  assert.equal(clamped.edges.length, 0);
  const capped = syntheticRecords({ seed: 0, nodeCount: 100000, degree: 99 });
  assert.equal(capped.nodes.length, 2000);
  assert.equal(capped.edges.length, (2000 - 12) * 12);
});

test("a full ingest document passes through the normaliser unchanged, with no notes", () => {
  const text = syntheticIngest({ seed: 5, nodeCount: 6, degree: 2 });
  const result = normaliseIngest(text, "synthetic");
  assert.deepEqual(result.notes, []);
  assert.equal(result.json, text);
});

test("a fixture's shorthand records are filled out to the full shape, and said so", () => {
  const fixture = JSON.stringify({
    about: "a chain",
    nodes: [{ id: "a" }, { id: "b" }],
    edges: [{ id: "ab", source: "a", target: "b" }],
  });
  const result = normaliseIngest(fixture, "dag/chain.json");
  const doc = JSON.parse(result.json) as IngestDoc;
  assert.equal(doc.nodes.length, 2);
  assert.equal(doc.nodes[0].id, "a");
  assert.equal(doc.nodes[0].kind, "record");
  assert.equal(doc.nodes[0].label, "a");
  assert.equal(doc.nodes[0].database_id, null);
  assert.equal(doc.edges[0].kind, "relation");
  assert.equal(doc.edges[0].directed, false);
  assert.ok(result.notes.some((note) => note.includes("about")));
  assert.ok(result.notes.some((note) => note.includes("defaulted")));
});

test("an edge `type` maps to a kind, and unknown record members are dropped with a note", () => {
  const fixture = JSON.stringify({
    nodes: [{ id: "a", role: "root", x: 1, y: 2 }],
    edges: [{ id: "ab", source: "a", target: "a", type: "parent" }],
  });
  const result = normaliseIngest(fixture, "hierarchy/tree.json");
  const doc = JSON.parse(result.json) as IngestDoc & { nodes: Record<string, unknown>[] };
  assert.deepEqual(Object.keys(doc.nodes[0]), [
    "id", "kind", "database_id", "source", "label", "group",
    "weight", "version", "has_note", "icon",
  ]);
  assert.equal(doc.edges[0].kind, "hierarchy");
  assert.equal(doc.edges[0].directed, true);
  assert.ok(result.notes.some((note) => note.includes("role")));
});

test("normalising twice is idempotent — the second pass is a no-op", () => {
  const once = normaliseIngest(JSON.stringify({ nodes: [{ id: "a" }], edges: [] }), "x.json");
  const twice = normaliseIngest(once.json, "x.json");
  assert.equal(twice.json, once.json);
  assert.deepEqual(twice.notes, []);
});

test("a document that is not JSON is refused, not half-read", () => {
  assert.throws(() => normaliseIngest("{ not json", "bad.json"), IngestRefusal);
  assert.throws(() => normaliseIngest("{ not json", "bad.json"), /bad.json: not JSON/);
});

test("a document with no nodes array is refused by name", () => {
  assert.throws(() => normaliseIngest('{"version":1}', "no-nodes.json"), /no-nodes.json: no `nodes` array/);
});

test("a version other than 1 is refused rather than guessed at", () => {
  assert.throws(() => normaliseIngest('{"version":2,"nodes":[],"edges":[]}', "v2.json"), /v2.json: unsupported version 2/);
});

test("a duplicate node id is refused, and the message names it", () => {
  assert.throws(
    () => normaliseIngest('{"nodes":[{"id":"a"},{"id":"a"}],"edges":[]}', "dupe.json"),
    /dupe.json: duplicate node id "a"/,
  );
});

test("an edge naming a node that is not there is refused, and the message names the end", () => {
  assert.throws(
    () => normaliseIngest('{"nodes":[{"id":"a"}],"edges":[{"id":"ab","source":"a","target":"z"}]}', "dangling.json"),
    /dangling.json: edge "ab" targets "z", which is not a node/,
  );
});

test("a record that is not an object, or has no id, is refused with its index", () => {
  assert.throws(() => normaliseIngest('{"nodes":[7],"edges":[]}', "r.json"), /r.json: nodes\[0\] is not an object/);
  assert.throws(() => normaliseIngest('{"nodes":[{"label":"x"}],"edges":[]}', "r.json"), /r.json: nodes\[0\] has no string `id`/);
  assert.throws(() => normaliseIngest('{"nodes":[{"id":"a"}],"edges":[{"id":"e"}]}', "r.json"), /r.json: edges\[0\] has no string `source`/);
});

test("a node kind the contract does not name is refused", () => {
  assert.throws(
    () => normaliseIngest('{"nodes":[{"id":"a","kind":"Record"}],"edges":[]}', "k.json"),
    /k.json: nodes\[0\].kind "Record" is not a node kind/,
  );
});

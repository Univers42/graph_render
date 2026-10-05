// What a parsed query means over one node, and what a filter document means as the two
// masks the painter already reads. Every field, every operator, every boolean shape and
// every `hiddenOf` branch is pinned here, on a graph built by hand so that each node
// differs from its neighbour on exactly one thing. The last section runs the same code
// over the fixture that carries what the sources do not.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { parseQuery } from "../src/console/parse.ts";
import type { Query, QueryField } from "../src/console/parse.ts";
import { matchesQuery, rowOf, type QueryRow } from "../src/console/queryMatch.ts";
import { hiddenOf, highlightOf } from "../src/look/visibleOf.ts";
import { normaliseIngest, type IngestEdge, type IngestNode } from "../src/source/ingest.ts";
import { UNGROUPED, metaOf, type Ends, type GraphMeta } from "../src/source/meta.ts";
import type { Filter } from "../src/state/settings.ts";

// Every filter member, filled: the mask is a function of the whole document, so a test
// naming only some of it is testing a document nobody can type.
const BASE: Filter = {
  query: "", text: "", hiddenKinds: [], hiddenGroups: [], orphans: false, existingOnly: false, minDegree: 0, relayout: false,
};
const ALL: readonly boolean[] = [true, true, true, true, true, true];

function filter(patch: Partial<Filter>): Filter {
  return { ...BASE, ...patch };
}

function field(name: QueryField, op: string, value: string): Query {
  return { kind: "field", field: name, op, value };
}

// Degrees are fixed here rather than measured: `hiddenOf` reads the column, not edges.
const META: GraphMeta = {
  nodeCount: 6,
  ids: ["a", "b", "c", "d", "e", "f"],
  labels: ["Alpha hub", "Beta note", "Gamma tag", "Delta db", "Epsilon rec", "Zeta note"],
  kinds: ["record", "note", "tag", "database", "record", "note"],
  groups: ["Core", "Tags", UNGROUPED],
  group: Uint16Array.of(0, 0, 1, 0, 2, 0),
  weight: Float32Array.of(1, 0.5, 0.25, 0, 0.5, 0.25),
  degree: Uint32Array.of(3, 1, 2, 0, 0, 0),
  maxDegree: 3,
  // Unix seconds a source's `updatedAt` carries; one node carries none, and reads as 0.
  versions: Float64Array.of(1700000000, 1600000000, 1700000000, 0, 1500000000, 1800000000),
  tags: [["one", "two"], ["two"], [], ["three"], [], ["ONE"]],
  dbs: ["db-1", "db-1", "db-2", "", "db-3", "db-1"],
  paths: ["src/a.md", "src/b.md", "", "src/d.md", "", "src/f.md"],
};

function every(query: Query): readonly boolean[] {
  return Array.from({ length: META.nodeCount }, (_, i) => matchesQuery(query, rowOf(META, i)));
}

function mask(bits: Uint8Array | null): readonly number[] {
  return [...(bits ?? [])];
}

test("a row is total: every column is there, empty when the graph carries nothing", () => {
  const full: QueryRow = {
    id: "a", label: "Alpha hub", kind: "record", tags: ["one", "two"], db: "db-1", path: "src/a.md",
    degree: 3, group: "Core", version: 1700000000,
  };
  assert.deepEqual(rowOf(META, 0), full);
  assert.deepEqual(rowOf(META, 2), {
    id: "c", label: "Gamma tag", kind: "tag", tags: [], db: "db-2", path: "",
    degree: 2, group: "Tags", version: 1700000000,
  });
  assert.deepEqual(rowOf(META, 4), {
    id: "e", label: "Epsilon rec", kind: "record", tags: [], db: "db-3", path: "",
    degree: 0, group: UNGROUPED, version: 1500000000,
  });
});

test("the empty query matches every node, and free text is a case-insensitive substring", () => {
  assert.deepEqual(every({ kind: "all" }), ALL);
  assert.deepEqual(every({ kind: "text", text: "hub" }), [true, false, false, false, false, false]);
  assert.deepEqual(every({ kind: "text", text: "NOTE" }), [false, true, false, false, false, true]);
  assert.deepEqual(every({ kind: "text", text: "" }), ALL);
  assert.deepEqual(every({ kind: "text", text: "zzz" }), ALL.map(() => false));
});

test("id, kind, db and path are case-insensitive equality, with and without the `=`", () => {
  assert.deepEqual(every(field("id", "=", "a")), [true, false, false, false, false, false]);
  assert.deepEqual(every(field("id", "", "A")), [true, false, false, false, false, false]);
  assert.deepEqual(every(field("kind", "=", "note")), [false, true, false, false, false, true]);
  assert.deepEqual(every(field("db", "=", "DB-1")), [true, true, false, false, false, true]);
  assert.deepEqual(every(field("db", "=", "")), [false, false, false, true, false, false]);
  assert.deepEqual(every(field("path", "=", "src/b.md")), [false, true, false, false, false, false]);
  assert.deepEqual(every(field("path", "=", "")), [false, false, true, false, true, false]);
});

test("a tag is membership, case-insensitive, with a leading `#` the user may type", () => {
  assert.deepEqual(every(field("tag", "=", "two")), [true, true, false, false, false, false]);
  assert.deepEqual(every(field("tag", "=", "TWO")), [true, true, false, false, false, false]);
  assert.deepEqual(every(field("tag", "", "#two")), [true, true, false, false, false, false]);
  assert.deepEqual(every(field("tag", "=", "#one")), [true, false, false, false, false, true]);
  assert.deepEqual(every(field("tag", "=", "three")), [false, false, false, true, false, false]);
  assert.deepEqual(every(field("tag", "=", "#")), ALL.map(() => false));
});

test("group is the group's own name, case-insensitive, and the ungrouped node has one too", () => {
  assert.deepEqual(every(field("group", "", "Core")), [true, true, false, true, false, true]);
  assert.deepEqual(every(field("group", "", "core")), [true, true, false, true, false, true]);
  assert.deepEqual(every(field("group", "=", "CORE")), [true, true, false, true, false, true]);
  assert.deepEqual(every(field("group", ">", "Core")), [true, true, false, true, false, true]);
  assert.deepEqual(every(field("group", "", "Tags")), [false, false, true, false, false, false]);
  assert.deepEqual(every(field("group", "", UNGROUPED)), [false, false, false, false, true, false]);
  assert.deepEqual(every(field("group", "", "nothing here")), ALL.map(() => false));
});

test("version compares numbers, like degree, and a node with none reads as 0", () => {
  assert.deepEqual(every(field("version", ">=", "1700000000")), [true, false, true, false, false, true]);
  assert.deepEqual(every(field("version", "<", "1700000000")), [false, true, false, true, true, false]);
  assert.deepEqual(every(field("version", ">", "1500000000")), [true, true, true, false, false, true]);
  assert.deepEqual(every(field("version", "=", "0")), [false, false, false, true, false, false]);
  assert.deepEqual(every(field("version", "<=", "1500000000")), [false, false, false, true, true, false]);
  assert.deepEqual(every(field("version", ">", "1.5")), [true, true, true, false, true, true]);
});

test("group and version combine with not and and", () => {
  const query: Query = { kind: "and", of: [
    { kind: "not", of: field("group", "", "ana") },
    field("version", ">", "1700000000"),
  ] };
  assert.deepEqual(every(query), [false, false, false, false, false, true]);
});

test("degree compares numbers: `>` `<` `>=` `<=` and `=`", () => {
  assert.deepEqual(every(field("degree", ">", "1")), [true, false, true, false, false, false]);
  assert.deepEqual(every(field("degree", "<", "1")), [false, false, false, true, true, true]);
  assert.deepEqual(every(field("degree", ">=", "1")), [true, true, true, false, false, false]);
  assert.deepEqual(every(field("degree", "<=", "0")), [false, false, false, true, true, true]);
  assert.deepEqual(every(field("degree", "=", "2")), [false, false, true, false, false, false]);
  assert.deepEqual(every(field("degree", "", "0")), [false, false, false, true, true, true]);
});

// Ponytail: `degree:>abc` cannot be a comparison, and every comparison against NaN is
// false, so it matches nothing. Direction it errs: the query looks stricter than the
// user meant, and says nothing. Escape hatch: `not (degree:>abc)` matches everything.
test("a degree value that is not a number matches no node at all", () => {
  assert.deepEqual(every(field("degree", ">", "abc")), ALL.map(() => false));
  assert.deepEqual(every(field("degree", "=", "1.5")), [false, false, false, false, false, false]);
});

// Ponytail: `kind:>record` is not a comparison of strings; it is read as equality,
// the only reading that can be right. Direction it errs: a nonsense operator matches
// more than the user expected. Escape hatch: the parser may refuse it instead.
test("an operator that is neither empty nor `=` falls back to equality", () => {
  assert.deepEqual(every(field("kind", ">", "tag")), [false, false, true, false, false, false]);
  assert.deepEqual(every(field("group", ">", "Tags")), [false, false, true, false, false, false]);
  assert.deepEqual(every(field("id", "!=", "a")), [true, false, false, false, false, false]);
  assert.deepEqual(every(field("tag", "has", "two")), [true, true, false, false, false, false]);
});

test("not negates, and a negation of a negation is the query", () => {
  const notKind: Query = { kind: "not", of: field("kind", "=", "note") };
  assert.deepEqual(every(notKind), [true, false, true, true, true, false]);
  assert.deepEqual(every({ kind: "not", of: notKind }), [false, true, false, false, false, true]);
  assert.deepEqual(every({ kind: "not", of: { kind: "all" } }), ALL.map(() => false));
});

test("and is all of them, or is any of them", () => {
  const tag = field("tag", "=", "two");
  const note = field("kind", "=", "note");
  assert.deepEqual(every({ kind: "and", of: [tag, note] }), [false, true, false, false, false, false]);
  assert.deepEqual(every({ kind: "and", of: [tag, field("kind", "=", "record")] }), [true, false, false, false, false, false]);
  assert.deepEqual(every({ kind: "and", of: [note, field("degree", "<", "1")] }), [false, false, false, false, false, true]);
  assert.deepEqual(every({ kind: "or", of: [tag, note] }), [true, true, false, false, false, true]);
  assert.deepEqual(every({ kind: "or", of: [field("kind", "=", "tag"), field("kind", "=", "database")] }), [false, false, true, true, false, false]);
  assert.deepEqual(every({ kind: "and", of: [{ kind: "all" }, note] }), [false, true, false, false, false, true]);
});

test("nothing filtered is nothing hidden, and `null` says so", () => {
  assert.equal(hiddenOf(META, BASE), null);
  assert.equal(hiddenOf(META, filter({ query: "", text: "   ", minDegree: 0 })), null);
  assert.equal(hiddenOf(META, filter({ relayout: true })), null);
  assert.equal(highlightOf(META, BASE), null);
  assert.equal(highlightOf(META, filter({ text: "  ", query: "" })), null);
});

test("free text hides every node whose label does not contain it, case-insensitively", () => {
  assert.deepEqual(mask(hiddenOf(META, filter({ text: "NOTE" }))), [1, 0, 1, 1, 1, 0]);
  assert.deepEqual(mask(hiddenOf(META, filter({ text: "  db " }))), [1, 1, 1, 0, 1, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ text: "zzz" }))), [1, 1, 1, 1, 1, 1]);
});

test("a hidden group hides its nodes, and a hidden kind hides its nodes", () => {
  assert.deepEqual(mask(hiddenOf(META, filter({ hiddenGroups: ["Core"] }))), [1, 1, 0, 1, 0, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ hiddenGroups: [UNGROUPED] }))), [0, 0, 0, 0, 1, 0]);
  assert.deepEqual(mask(hiddenOf(META, filter({ hiddenKinds: ["tag"] }))), [0, 0, 1, 0, 0, 0]);
  assert.deepEqual(mask(hiddenOf(META, filter({ hiddenKinds: ["note", "database"] }))), [0, 1, 0, 1, 0, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ hiddenGroups: ["nothing here"] }))), [0, 0, 0, 0, 0, 0]);
});

test("minDegree hides the quiet nodes, and orphans and existingOnly hide the same set", () => {
  assert.deepEqual(mask(hiddenOf(META, filter({ minDegree: 2 }))), [0, 1, 0, 1, 1, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ minDegree: 3 }))), [0, 1, 1, 1, 1, 1]);
  const orphans = mask(hiddenOf(META, filter({ orphans: true })));
  assert.deepEqual(orphans, [0, 0, 0, 1, 1, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ existingOnly: true }))), orphans);
});

test("the query hides every node it does not match", () => {
  assert.deepEqual(mask(hiddenOf(META, filter({ query: "kind:tag" }))), [1, 1, 0, 1, 1, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ query: "tag:two" }))), [0, 0, 1, 1, 1, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ query: "degree:>=2" }))), [0, 1, 0, 1, 1, 1]);
});

// Ponytail: a query that does not parse is dropped rather than obeyed. Failing input
// `kind:tag AND`: the studio would blank the canvas and show no reason for it, which
// reads as a broken graph rather than a broken line. Direction it errs: the filter is
// ignored, so more is drawn than the user asked for. Escape hatch: the console reports
// the refusal beside the line, which is where the user is already looking. Whether the
// refusal reaches here or the parser reads the line loosely, the canvas is never empty.
test("a query that does not parse hides nothing rather than everything", () => {
  for (const broken of ["((", "kind:tag AND", "and or not", "kind:"]) {
    assert.equal(mask(hiddenOf(META, filter({ query: broken }))).reduce((n, bit) => n + bit, 0), 0, broken);
  }
});

test("criteria combine: any one of them hides the node", () => {
  assert.deepEqual(mask(hiddenOf(META, filter({ query: "db:db-1", text: "hub" }))), [0, 1, 1, 1, 1, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ hiddenKinds: ["tag"], minDegree: 2 }))), [0, 1, 1, 1, 1, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ query: "db:db-1", hiddenGroups: ["Core"], orphans: true }))), [1, 1, 1, 1, 1, 1]);
  assert.deepEqual(mask(hiddenOf(META, filter({ query: "db:db-1", hiddenGroups: ["Tags"] }))), [0, 0, 1, 1, 1, 0]);
  assert.deepEqual(
    mask(hiddenOf(META, filter({ query: "kind:note", hiddenKinds: ["note"], existingOnly: true }))),
    [1, 1, 1, 1, 1, 1],
  );
});

test("the search highlights exactly what it matches, and the query wins over the text", () => {
  assert.deepEqual(mask(highlightOf(META, filter({ text: "note" }))), [0, 1, 0, 0, 0, 1]);
  assert.deepEqual(mask(highlightOf(META, filter({ text: " NOTE " }))), [0, 1, 0, 0, 0, 1]);
  assert.deepEqual(mask(highlightOf(META, filter({ text: "zzz" }))), [0, 0, 0, 0, 0, 0]);
  assert.deepEqual(mask(highlightOf(META, filter({ text: "hub", query: "kind:tag" }))), [0, 0, 1, 0, 0, 0]);
  assert.deepEqual(mask(highlightOf(META, filter({ query: "tag:#one" }))), [1, 0, 0, 0, 0, 1]);
});

test("both masks are one byte per node", () => {
  const hidden = hiddenOf(META, filter({ orphans: true }));
  const lit = highlightOf(META, filter({ text: "note" }));
  assert.ok(hidden !== null && lit !== null);
  assert.equal(hidden.length, META.nodeCount);
  assert.equal(lit.length, META.nodeCount);
  assert.equal(hidden[0], 0);
  assert.equal(lit[1], 1);
});

// The fixture: what the wasm contract cannot carry. Read as a file, because a
// generated document would only pin the generator.
const FIXTURE = new URL("fixtures/keys.json", import.meta.url);

async function fixtureMeta(): Promise<GraphMeta> {
  const { doc } = normaliseIngest(await readFile(FIXTURE, "utf8"), "fixtures/keys.json");
  return metaOf(doc.nodes, doc.nodes.map((node) => node.id), endsOf(doc.nodes, doc.edges));
}

function found(meta: GraphMeta, text: string): readonly string[] {
  const query = parseQuery(text);
  return Array.from({ length: meta.nodeCount }, (_, i) => i)
    .filter((i) => matchesQuery(query, rowOf(meta, i)))
    .map((i) => meta.ids[i] ?? "");
}

test("the fixture normalises, and its comment is said to be dropped", async () => {
  const { doc, notes } = normaliseIngest(await readFile(FIXTURE, "utf8"), "fixtures/keys.json");
  assert.equal(doc.nodes.length, 12);
  assert.equal(doc.edges.length, 10);
  assert.ok(notes.includes("dropped annotation `_comment`"));
});

test("the fixture's three columns are exactly these, in the document's order", async () => {
  const meta = await fixtureMeta();
  assert.deepEqual(meta.tags, [
    ["index"], ["atlas", "index"], ["atlas"], ["draft", "atlas"], ["draft"], [], ["colour"],
    ["colour"], ["atlas", "colour"], ["draft"], [], [],
  ]);
  assert.deepEqual(meta.dbs, [
    "db-core", "db-core", "db-core", "db-notes", "db-notes", "db-notes",
    "db-tags", "db-tags", "db-stamp", "", "db-core", "",
  ]);
  assert.deepEqual(meta.paths, [
    "vault/index.json", "vault/atlas.json", "", "notes/loop.md", "notes/two.md", "notes/orphan.md",
    "", "", "drafts/stamp.json", "drafts/one.json", "", "notes/lonely.md",
  ]);
});

test("the fixture has the four shapes every filter needs", async () => {
  const meta = await fixtureMeta();
  assert.deepEqual([...meta.degree], [3, 3, 3, 3, 2, 0, 2, 2, 2, 0, 0, 0]);
  assert.equal(meta.degree.filter((degree) => degree === 0).length, 4);
  assert.deepEqual([...new Set(meta.kinds)].sort(), ["database", "note", "record", "tag"]);
  assert.equal(new Set(meta.dbs).size, 5);
  assert.equal(meta.tags.filter((tags) => tags.length === 2).length, 3);
  assert.equal(meta.tags.filter((tags) => tags.length === 0).length, 3);
  assert.equal(meta.paths.filter((path) => path === "").length, 4);
  assert.equal(meta.labels.filter((label) => label.includes(" ")).length, 10);
});

test("queries parsed from text find what the fixture was built to find", async () => {
  const meta = await fixtureMeta();
  assert.deepEqual(found(meta, "tag:#atlas"), ["n-atlas", "n-bridge", "n-note-loop", "n-stamp"]);
  assert.deepEqual(found(meta, "db:db-notes"), ["n-note-loop", "n-note-two", "n-note-orphan"]);
  assert.deepEqual(found(meta, "path:notes/loop.md"), ["n-note-loop"]);
  assert.deepEqual(found(meta, "kind:tag"), ["t-red", "t-blue"]);
  assert.deepEqual(found(meta, "degree:>2"), ["n-vault", "n-atlas", "n-bridge", "n-note-loop"]);
  assert.deepEqual(found(meta, "atlas"), ["n-atlas"]);
});

// A label with a space needs a quoted value; what the parser makes of the quotes is the
// parser's business, and what the quoted text then means is pinned here.
test("a quoted value is just text, spaces and all", async () => {
  const meta = await fixtureMeta();
  assert.deepEqual(found(meta, '"Vault index"'), ["n-vault"]);
  const quoted: Query = { kind: "text", text: "Vault index" };
  assert.deepEqual(meta.ids.filter((_, i) => matchesQuery(quoted, rowOf(meta, i))), ["n-vault"]);
});

test("the fixture's orphans are exactly the four the filter hides", async () => {
  const meta = await fixtureMeta();
  const hidden = mask(hiddenOf(meta, filter({ orphans: true })));
  assert.deepEqual(meta.ids.filter((_, i) => hidden[i] === 1), ["n-note-orphan", "n-draft-only", "n-vault-mirror", "n-note-lonely"]);
  assert.deepEqual(mask(hiddenOf(meta, filter({ query: "tag:#colour" }))), [1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1, 1]);
  assert.deepEqual(mask(highlightOf(meta, filter({ text: "vault" }))), [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0]);
});

/** The snapshot a build produces for this document: node index order, so the columns are
 *  read against known indices rather than a permutation nobody here can predict. */
function endsOf(nodes: readonly IngestNode[], edges: readonly IngestEdge[]): Ends {
  const slot = new Map(nodes.map((node, i) => [node.id, i]));
  return {
    source: Uint32Array.from(edges, (edge) => slot.get(edge.source) ?? 0),
    target: Uint32Array.from(edges, (edge) => slot.get(edge.target) ?? 0),
  };
}

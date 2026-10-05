// The last section of the query-match suite, over the fixture that carries what the sources
// do not: the columns `tests/query-match.test.ts` builds by hand, read from a real document
// instead. Split into its own file so both stay under the house line limit.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

import { type Query, parseQuery } from "../src/console/parse.ts";
import { matchesQuery, rowOf } from "../src/console/queryMatch.ts";
import { hiddenOf, highlightOf } from "../src/look/visibleOf.ts";
import { normaliseIngest, type IngestEdge, type IngestNode } from "../src/source/ingest.ts";
import { metaOf, type Ends, type GraphMeta } from "../src/source/meta.ts";
import type { Filter } from "../src/state/settings.ts";

const BASE: Filter = {
  query: "", text: "", hiddenKinds: [], hiddenGroups: [], orphans: false, existingOnly: false, minDegree: 0, relayout: false,
};

function filter(patch: Partial<Filter>): Filter {
  return { ...BASE, ...patch };
}

function mask(bits: Uint8Array | null): readonly number[] {
  return [...(bits ?? [])];
}

// Read as a file, because a generated document would only pin the generator.
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

// The fixture names no version, so every node reads as 0: the column exists and is honest.
test("a document with no version reads as 0 on every node", async () => {
  const meta = await fixtureMeta();
  assert.deepEqual([...new Set(meta.versions)], [0]);
  assert.deepEqual(found(meta, "version:<1"), meta.ids);
  assert.deepEqual(found(meta, "version:>=1"), []);
  assert.deepEqual(found(meta, "version:=0"), meta.ids);
});

test("the fixture's groups are the four the document names, read by name", async () => {
  const meta = await fixtureMeta();
  assert.deepEqual([...new Set(meta.groups)].sort(), ["Core", "Drafts", "Notes", "Tags"]);
  assert.deepEqual(found(meta, "group:drafts"), ["n-stamp", "n-draft-only"]);
  assert.deepEqual(found(meta, "group:DRAFTS"), ["n-stamp", "n-draft-only"]);
  assert.deepEqual(found(meta, 'group:"nothing here"'), []);
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
/**
 * Row `host-api-unit` for the additive `loadColumns` verb (`docs/contract/host-api.md` verdict 7
 * and `docs/contract/ingest-columns.md`): the `LoadResult` it resolves with, the
 * `Document.nodes` the worker builds from the rows, the `TypeError` each refused field gets, the
 * disconnected element, and that a host's columns are never persisted.
 */
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { hostVerbs } from "../src/host/api.ts";
import { createPreviews } from "../src/host/previews.ts";
import { watchHost } from "../src/host/watch.ts";
import type { MotorClient } from "../src/motor/client.ts";
import { documentFor } from "../src/motor/documents.ts";
import type { ColumnRowsLike } from "../src/source/synthetic-columns.ts";
import { sourceKey } from "../src/state/persist.ts";
import type { StudioState } from "../src/state/model.ts";
import type { Store } from "../src/state/store.ts";
import { SCRIPTED_META, desk, scriptedClient } from "./desk.ts";

/** `u32::MAX`: the field is absent, as the JSON reader's `null` is (`ingest-columns.md:51-53`). */
const ABSENT = 0xffff_ffff;
/** The string table, in the indices the columns below name. */
const STRINGS = ["a", "b", "c", "record", "file", "relation", "e0", "e1"];
const ID = 0;
const KIND = 3;
const SOURCE = 4;
const EDGE_KIND = 5;
const EDGE_ID = 6;

/**
 * Three nodes and two edges, as `ColumnRowsLike`: one `u32` column per field in the contract's
 * own order (`docs/contract/ingest-columns.md:22-37`), so cell `count + r` is row `r` of the
 * second column and the arrays hold eight columns of `count` and of `edges` rows.
 */
function columns(): ColumnRowsLike {
  const count = 3;
  const edges = 2;
  const nodeCells = new Uint32Array(8 * count);
  nodeCells.set([ID, ID + 1, ID + 2], 0);
  nodeCells.fill(KIND, count, 2 * count);
  nodeCells.fill(ABSENT, 2 * count, 3 * count);
  nodeCells.fill(SOURCE, 3 * count, 4 * count);
  nodeCells.set([ID, ID + 1, ID + 2], 4 * count);
  nodeCells.fill(ABSENT, 5 * count, 7 * count);
  const edgeCells = new Uint32Array(8 * edges);
  edgeCells.set([EDGE_ID, EDGE_ID + 1], 0);
  edgeCells.set([0, 1], 2);
  edgeCells.set([1, 2], 4);
  edgeCells.fill(EDGE_KIND, 6, 8);
  edgeCells.fill(ABSENT, 10, 12);
  return {
    strings: STRINGS,
    nodeCells,
    edgeCells,
    weights: Float64Array.of(0.5, 0.5, 0.5),
    versions: new Float64Array(count),
    strengths: Float64Array.of(1, 1),
  };
}

/** One run's verbs over a scripted motor, listening the way a host page listens. */
async function started(client: MotorClient = scriptedClient()): Promise<{
  readonly heard: string[];
  readonly columns: (rows: unknown) => Promise<unknown>;
  readonly store: Store<StudioState>;
}> {
  const scripted = client;
  const counting = {
    ...scripted,
    layout: async (layout: string, post: string | null) =>
      ({ ...(await scripted.layout(layout, post)), meta: { ...SCRIPTED_META } }),
  };
  const made = desk(counting);
  const entry = await made.studio.start();
  assert.equal(entry.ok, true, entry.message);
  const host = new EventTarget();
  const heard: string[] = [];
  for (const name of ["graph-load", "graph-error"]) {
    host.addEventListener(name, (event) => {
      if (event instanceof CustomEvent) heard.push(`${name} ${JSON.stringify(event.detail)}`);
    });
  }
  watchHost({ host, store: made.studio.store, view: { on: () => () => undefined }, previews: createPreviews({ resolver: () => null }) });
  const verbs = hostVerbs(host, made.studio, Promise.resolve());
  return { heard, columns: (rows) => verbs.loadColumns(rows), store: made.studio.store };
}

test("loadColumns resolves with the counts and no notes, and graph-load says the same once", async () => {
  const subject = await started();
  const result = await subject.columns(columns());
  assert.deepEqual(result, { nodes: 3, edges: 2, notes: [] });
  assert.deepEqual(subject.heard, ['graph-load {"nodes":3,"edges":2,"notes":[]}']);
});

test("the worker builds Document.nodes from the rows, in row order, and assembles the bytes itself", async () => {
  const document = await documentFor(
    { kind: "columns", name: "host", rows: columns(), host: true },
    "",
    () => Promise.reject(new Error("no fixture is fetched on this path")),
    () => new Uint8Array(7),
  );
  const nodes = document.nodes;
  assert.deepEqual(nodes.map((node) => node.id), ["a", "b", "c"]);
  assert.deepEqual(nodes.map((node) => node.label), ["a", "b", "c"]);
  assert.deepEqual(nodes.map((node) => node.kind), ["record", "record", "record"]);
  assert.deepEqual(nodes.map((node) => node.source), ["file", "file", "file"]);
  assert.deepEqual(nodes.map((node) => node.weight), [0.5, 0.5, 0.5]);
  const first = nodes[0];
  assert.ok(first !== undefined, "one node per row, so the first exists");
  assert.equal(first.database_id, null);
  assert.equal(first.group, null);
  assert.equal(first.has_note, false);
  assert.equal(document.edgeCount, 2);
  assert.deepEqual(document.notes, []);
  assert.equal(document.payload.kind, "columns");
});

test("negative control: with the id column swapped the same reading does not hold", async () => {
  const rows = columns();
  rows.nodeCells[0] = ID + 1;
  const document = await documentFor(
    { kind: "columns", name: "host", rows, host: true }, "", () => Promise.reject(new Error("none")),
    () => new Uint8Array(0),
  );
  assert.notDeepEqual(document.nodes.map((node) => node.id), ["a", "b", "c"]);
});

/** Every field the boundary refuses, with the word the `TypeError` has to name. */
const REFUSED: readonly (readonly [string, () => unknown, string])[] = [
  ["a value that is not an object", () => 42, "rows"],
  ["no strings table", () => ({ ...columns(), strings: "a" }), "strings"],
  ["nodeCells that is not a Uint32Array", () => ({ ...columns(), nodeCells: [0] }), "nodeCells"],
  ["edgeCells that is not a Uint32Array", () => ({ ...columns(), edgeCells: new Uint16Array(16) }), "edgeCells"],
  ["weights that is not a Float64Array", () => ({ ...columns(), weights: new Float32Array(3) }), "weights"],
  ["versions that is not a Float64Array", () => ({ ...columns(), versions: new Float32Array(3) }), "versions"],
  ["strengths that is not a Float64Array", () => ({ ...columns(), strengths: new Float32Array(2) }), "strengths"],
  ["nodeCells a row short", () => ({ ...columns(), nodeCells: new Uint32Array(23) }), "nodeCells"],
  ["edgeCells a row short", () => ({ ...columns(), edgeCells: new Uint32Array(15) }), "edgeCells"],
  ["weights that disagree with the node rows", () => ({ ...columns(), weights: new Float64Array(2) }), "weights"],
  ["versions that disagree with the node count", () => ({ ...columns(), versions: new Float64Array(2) }), "versions"],
  ["strengths that disagree with the edge rows", () => ({ ...columns(), strengths: new Float64Array(3) }), "strengths"],
];

for (const [what, given, field] of REFUSED) {
  test(`loadColumns refuses ${what} with a TypeError naming ${field}, and nothing is dispatched`, async () => {
    const subject = await started();
    const before = subject.store.get().settings.source;
    const refused: unknown = await subject.columns(given()).then(() => null, (error: unknown) => error);
    assert.ok(refused instanceof TypeError, `${what} was not a TypeError`);
    assert.match(refused.message, new RegExp(`\\b${field}\\b`), refused.message);
    assert.deepEqual(subject.heard, [], "a call-boundary refusal has never emitted a graph-error");
    assert.deepEqual(subject.store.get().settings.source, before);
  });
}

/**
 * A motor that refuses the host's own load the way the wasm refuses a columnar document whose
 * node id is taken: the sibling class of `BuildRefusedError`, with the ABI code behind it.
 */
function columnsRefusing(): MotorClient {
  const scripted = scriptedClient();
  let loads = 0;
  const refused = Object.assign(new Error("the node id is already taken"), {
    name: "ColumnsRefusedError", code: 23, codeName: "ColumnsInvalid",
  });
  // The first load is the studio's own start; the host's is the second.
  return { ...scripted, load: (source) => (loads++ === 0 ? scripted.load(source) : Promise.reject(refused)) };
}

test("a refused loadColumns rejects as ColumnsRefusedError, and its graph-error carries the code", async () => {
  const subject = await started(columnsRefusing());
  const refused: unknown = await subject.columns(columns()).then(() => null, (error: unknown) => error);
  assert.ok(refused instanceof Error);
  assert.equal(refused.name, "ColumnsRefusedError");
  assert.deepEqual(subject.heard, ['graph-error {"error":"code 23 (ColumnsInvalid)","message":"the node id is already taken"}']);
});

const ELEMENT = readFileSync(new URL("../src/element.ts", import.meta.url), "utf8");

/** The `loadColumns` method's body line, the one that has to refuse at once. */
function elementColumnar(source: string): string {
  const lines = source.split("\n");
  const at = lines.findIndex((line) => line.includes("loadColumns(rows:"));
  return lines.slice(at, at + 3).join("\n");
}

test("loadColumns on a disconnected element rejects at once, as loadGraph does", () => {
  const body = elementColumnar(ELEMENT);
  assert.match(body, /this\.#mounted === null \? notConnected\(\)/, body);
  assert.match(body, /verbs\.loadColumns\(rows\)/, body);
});

test("negative control: an optional chain on the disconnected path is not the same reading", () => {
  const mutated = ELEMENT.replace(
    "this.#mounted === null ? notConnected() : this.#mounted.verbs.loadColumns(rows)",
    "this.#mounted?.verbs.loadColumns(rows)",
  );
  assert.doesNotMatch(elementColumnar(mutated), /notConnected\(\)/);
});

test("the settings source of a host's columns is not persisted", async () => {
  const subject = await started();
  await subject.columns(columns());
  const source = subject.store.get().settings.source;
  assert.equal(source.kind, "columns");
  assert.equal(sourceKey(source), null);
});
/**
 * The synthetic generator's columnar document, against the records it replaces.
 *
 * The claim being pinned is that `syntheticColumns(spec)` and `syntheticRecords(spec)` are the
 * same graph: every field, every row, in order, weights compared with `Object.is`. The only
 * thing standing between the studio's open path and the JSON one is this differential, because
 * the studio no longer builds the JSON to compare against at run time.
 *
 * Three things are checked, in order of how badly a regression would hurt:
 *   * the corpus — 64 specs across both shapes, two seeds, four node counts and four degrees,
 *     decoding to exactly `syntheticRecords`, including the awkward `degree >= count` corners
 *     where a generator must draw nothing at all;
 *   * end to end — the snapshot bytes after `layout.forceatlas2.barnes_hut` are identical
 *     whether the graph arrived as JSON or as columns;
 *   * the negative control — a document mutated in its bytes must fail the same comparison,
 *     or the comparison is not looking at what it claims to look at.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { assembleColumns, createMotor } from "../../../crates/graph-sdk-js/src/index.ts";
import { EDGE_KINDS, NODE_KINDS } from "../src/source/ingest.ts";
import type { IngestEdge, IngestNode } from "../src/source/ingest.ts";
import { syntheticIngest, syntheticRecords } from "../src/source/synthetic.ts";
import type { SyntheticShape, SyntheticSpec } from "../src/source/synthetic.ts";
import { syntheticColumns } from "../src/source/synthetic-columns.ts";
import { SKIP, WASM } from "./motor.ts";

/** `u32::MAX` in an optional column: absent, exactly as the JSON reader's `null`. */
const ABSENT = 0xffff_ffff;
/** A closed form, no RNG, so one graph always lays out to the same bytes. */
const FORCE = "layout.forceatlas2.barnes_hut";

/** Every section's start, recomputed from the header rather than from a constant — so a
 *  document whose sections do not tile its buffer is a wrong answer here, not a wrong read. */
function sections(bytes: Uint8Array) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const u32 = (at: number): number => view.getUint32(at, true);
  assert.equal(u32(0), 0x31434d47, "the magic is GMC1");
  assert.equal(u32(4), 1, "version 1");
  assert.equal(u32(24), 0, "reserved word 6 is zero");
  assert.equal(u32(28), 0, "reserved word 7 is zero");
  const nodeCount = u32(8);
  const edgeCount = u32(12);
  const stringCount = u32(16);
  const offsetsAt = 32;
  const blobAt = offsetsAt + 4 * (stringCount + 1);
  const head = blobAt + u32(20);
  const floatsAt = head + ((8 - (head % 8)) % 8);
  const intsAt = floatsAt + 8 * (2 * nodeCount + edgeCount);
  assert.equal(intsAt + 4 * 8 * (nodeCount + edgeCount), bytes.byteLength,
    "the declared sections sum to the buffer length");
  return { bytes, view, u32, nodeCount, edgeCount, stringCount, offsetsAt, blobAt, floatsAt, intsAt };
}

type Layout = ReturnType<typeof sections>;

/** The four ways a cell is read back: a string index, an optional one, and the two `u32`
 *  columns. Nothing here trusts the encoder — every read is bounds-checked and named. */
function readers(s: Layout) {
  const utf8 = new TextDecoder();
  const slice = (i: number): string => utf8.decode(s.bytes.subarray(
    s.blobAt + s.u32(s.offsetsAt + 4 * i), s.blobAt + s.u32(s.offsetsAt + 4 * (i + 1)),
  ));
  const table = Array.from({ length: s.stringCount }, (_, i) => slice(i));
  const named = (index: number, field: string): string => {
    assert.ok(index < s.stringCount, `${field}: index ${index} names no string`);
    return table[index] ?? "";
  };
  const optional = (index: number, field: string): string | null =>
    index === ABSENT ? null : named(index, field);
  const nodeCell = (column: number, row: number): number => s.u32(s.intsAt + 4 * (column * s.nodeCount + row));
  const edgeCell = (column: number, row: number): number =>
    s.u32(s.intsAt + 4 * (8 * s.nodeCount + column * s.edgeCount + row));
  return { named, optional, nodeCell, edgeCell };
}

type Readers = ReturnType<typeof readers>;

/** The node records. A kind that resolves to nothing is refused, exactly as the motor's own
 *  reader refuses it: a document that decoded to an unknown kind has not decoded. */
function decodeNodes(s: Layout, read: Readers): IngestNode[] {
  return Array.from({ length: s.nodeCount }, (_, r): IngestNode => {
    const name = read.named(read.nodeCell(1, r), "node kind");
    const kind = NODE_KINDS.find((candidate) => candidate === name);
    assert.ok(kind !== undefined, `node ${r}: ${name} is not a node kind`);
    return {
      id: read.named(read.nodeCell(0, r), "node id"),
      kind,
      database_id: read.optional(read.nodeCell(2, r), "node database"),
      source: read.named(read.nodeCell(3, r), "node source"),
      label: read.named(read.nodeCell(4, r), "node label"),
      group: read.optional(read.nodeCell(5, r), "node group"),
      weight: s.view.getFloat64(s.floatsAt + 8 * r, true),
      version: s.view.getFloat64(s.floatsAt + 8 * (s.nodeCount + r), true),
      has_note: read.nodeCell(7, r) === 1,
      icon: read.optional(read.nodeCell(6, r), "node icon"),
    };
  });
}

/** The edge records. An endpoint is a node **row**, resolved through the nodes above; a row
 *  past the last node is refused rather than read as some other node. */
function decodeEdges(s: Layout, read: Readers, nodes: IngestNode[]): IngestEdge[] {
  const endpoint = (column: number, row: number): string => {
    const at = read.edgeCell(column, row);
    const node = at < s.nodeCount ? nodes[at] : undefined;
    assert.ok(node !== undefined, `edge column ${column}, row ${row}: ${at} is not a node`);
    return node.id;
  };
  return Array.from({ length: s.edgeCount }, (_, j): IngestEdge => {
    const name = read.named(read.edgeCell(3, j), "edge kind");
    const kind = EDGE_KINDS.find((candidate) => candidate === name);
    assert.ok(kind !== undefined, `edge ${j}: ${name} is not an edge kind`);
    return {
      id: read.named(read.edgeCell(0, j), "edge id"),
      source: endpoint(1, j),
      target: endpoint(2, j),
      kind,
      label: read.named(read.edgeCell(4, j), "edge label"),
      strength: s.view.getFloat64(s.floatsAt + 8 * (2 * s.nodeCount + j), true),
      directed: read.edgeCell(6, j) === 1,
      record_id: read.optional(read.edgeCell(5, j), "edge record_id"),
      child_first: read.edgeCell(7, j) === 1,
    };
  });
}

/** Bytes back into the records `syntheticRecords` makes, resolving every string index and
 *  every endpoint row to its id. Test-only: the motor's own decoder is the Rust one. */
function decode(bytes: Uint8Array): { nodes: IngestNode[]; edges: IngestEdge[] } {
  const s = sections(bytes);
  const read = readers(s);
  const nodes = decodeNodes(s, read);
  return { nodes, edges: decodeEdges(s, read, nodes) };
}

/** Both shapes, as their own type rather than by assertion: the list is what every test in
 *  this file loops over, and a typo in it should not type-check. */
const BOTH_SHAPES: readonly SyntheticShape[] = ["random", "vault"];

/** Every spec the differential runs: both shapes, two seeds, and node counts and degrees whose
 *  corners matter — 2 and 3 nodes with degree 12 is `degree >= count`, where `random` must draw
 *  no edges at all rather than name an endpoint that does not exist yet. Flat maps, not nested
 *  loops: four levels of block is past the house's nesting limit and reads no better. */
function corpus(): SyntheticSpec[] {
  return BOTH_SHAPES.flatMap((shape) => [1, 7].flatMap((seed) =>
    [2, 3, 50, 2000].flatMap((nodeCount) =>
      [0, 1, 3, 12].map((degree) => ({ seed, nodeCount, degree, shape })))));
}

test("the columns document decodes to exactly the records the JSON path makes", () => {
  const specs = corpus();
  assert.equal(specs.length, 64, "the corpus is 2 shapes x 2 seeds x 4 counts x 4 degrees");
  for (const spec of specs) {
    const records = syntheticRecords(spec);
    const decoded = decode(assembleColumns(syntheticColumns(spec).rows));
    assert.deepStrictEqual(decoded, records, JSON.stringify(spec));
  }
});

test("a spec whose degree exceeds its node count draws what the formula says", () => {
  // The edge count is a formula over the loops, and the columns arrays are allocated from it.
  // A generator that drew more would write past its own arrays, so the count is checked rather
  // than inferred from the fact that the document decoded.
  //
  // The two shapes disagree here, which is why both are spelled out: `random` starts its loop
  // at `max(1, degree)`, so `degree >= count` means it draws *nothing* — there is no earlier
  // node to name. `vault` starts at node 1 and skips only node 0, so `degree * (count - 1)`.
  const cases: ReadonlyArray<readonly [SyntheticShape, number, number, number]> = [
    ["random", 2, 12, 0], ["random", 3, 12, 0], ["random", 12, 12, 0], ["random", 3, 1, 2],
    ["vault", 2, 12, 12], ["vault", 3, 12, 24], ["vault", 12, 12, 132], ["vault", 3, 1, 2],
  ];
  for (const [shape, nodeCount, degree, edges] of cases) {
    const spec: SyntheticSpec = { seed: 1, nodeCount, degree, shape };
    assert.equal(syntheticColumns(spec).edgeCount, edges, `${shape} ${nodeCount}/${degree}`);
    assert.equal(syntheticRecords(spec).edges.length, edges, "and the records agree");
  }
});

test("the same graph arrives by either path, byte for byte, after a layout", { skip: SKIP }, async () => {
  const motor = await createMotor(WASM ?? new Uint8Array(0));
  for (const shape of BOTH_SHAPES) {
    const spec: SyntheticSpec = { seed: 1, nodeCount: 2000, degree: 3, shape };
    const viaJson = motor.build(syntheticIngest(spec));
    const viaColumns = motor.buildColumns(assembleColumns(syntheticColumns(spec).rows));
    motor.layout(viaJson, FORCE);
    motor.layout(viaColumns, FORCE);
    const json = Buffer.from(motor.toBytes(viaJson));
    const columns = Buffer.from(motor.toBytes(viaColumns));
    assert.equal(Buffer.compare(json, columns), 0, `${shape}: identical snapshot bytes`);
  }
});

test("swapping two edges' target rows fails the same comparison, both ways", { skip: SKIP }, async () => {
  // The negative control. If the differential and the end-to-end comparison both still passed on
  // a document whose edges point somewhere else, they would be passing on the encoding rather
  // than on the graph, and every other assertion in this file would be worthless.
  const spec: SyntheticSpec = { seed: 1, nodeCount: 2000, degree: 3, shape: "random" };
  const columns = syntheticColumns(spec);
  const bytes = assembleColumns(columns.rows);
  // The `edge target` column, cell 0: eight node columns, then edge id, source, target.
  const at = sections(bytes).intsAt + 4 * (8 * columns.rows.weights.length + 2 * columns.edgeCount);
  const swapped = bytes.slice();
  const view = new DataView(swapped.buffer);
  const first = view.getUint32(at, true);
  view.setUint32(at, view.getUint32(at + 4, true), true);
  view.setUint32(at + 4, first, true);

  assert.notDeepStrictEqual(decode(swapped), syntheticRecords(spec), "the records differ");

  const motor = await createMotor(WASM ?? new Uint8Array(0));
  const run = (document: Uint8Array): Buffer => {
    const handle = motor.buildColumns(document);
    motor.layout(handle, FORCE);
    return Buffer.from(motor.toBytes(handle));
  };
  assert.notEqual(Buffer.compare(run(bytes), run(swapped)), 0, "the snapshot bytes differ");
});

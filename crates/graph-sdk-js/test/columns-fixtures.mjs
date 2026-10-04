// Shared fixtures for the `gm_build_columns` split: the release artifact path, the
// awkward-values document, and the row/string builders. The tests themselves are in the
// sibling columns-*.test.mjs files.
//
// The module is read from `target/wasm32-unknown-unknown/release/graph_wasm.wasm`
// (`harness/sdk-smoke.mjs` is the same path).
//
// Run: `node --test --experimental-strip-types crates/graph-sdk-js/test/`.

import { readFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";

import { createMotor } from "../src/index.ts";
import { resetForTests } from "../src/wasm.ts";

export const WASM = fileURLToPath(
  new URL("../../../target/wasm32-unknown-unknown/release/graph_wasm.wasm", import.meta.url),
);

/** A small document with the awkward values: absent optionals, `-0.0`, a subnormal, a
 *  multi-byte id, a `child_first` edge — every field class `docs/contract/ingest-columns.md`
 *  can carry, in five nodes and five edges. */
export function document() {
  return {
    version: 1,
    nodes: [
      { id: "n-0", kind: "record", database_id: "db-0", source: "studio", label: "Graph notes", group: "Epsilon", weight: -0.0, version: 0.0, has_note: true, icon: "🌿" },
      { id: "n-1", kind: "note", database_id: null, source: "studio", label: "", group: null, weight: 1.0, version: 3.0, has_note: false, icon: null },
      { id: "n-2", kind: "database", database_id: "db-0", source: "studio", label: "db", group: null, weight: 0.25, version: 0.0, has_note: false, icon: null },
      { id: "n-3", kind: "tag", database_id: null, source: "studio", label: "tag", group: "Epsilon", weight: 0.5, version: 0.0, has_note: false, icon: null },
      { id: "n-é", kind: "record", database_id: "db-0", source: "studio", label: "multi", group: null, weight: 5e-324, version: -1.5, has_note: false, icon: null },
    ],
    edges: [
      { id: "e-0", source: "n-0", target: "n-1", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null, child_first: false },
      { id: "e-1", source: "n-1", target: "n-0", kind: "note_of", label: "of", strength: 0.5, directed: true, record_id: "rec-7", child_first: false },
      { id: "e-2", source: "n-3", target: "n-2", kind: "tag", label: "tag", strength: 0.5, directed: false, record_id: null, child_first: false },
      { id: "e-3", source: "n-1", target: "n-1", kind: "note_link", label: "link", strength: 0.5, directed: false, record_id: null, child_first: false },
      { id: "e-4", source: "n-é", target: "n-2", kind: "hierarchy", label: "h", strength: 0.5, directed: true, record_id: null, child_first: true },
    ],
  };
}

/** `layout.grid`: a closed form, no RNG, so two runs of the same graph are the same bytes.
 *  `layout.random` is the obvious thing to reach for and the wrong one — a layout with
 *  randomness makes every byte comparison a coin toss. */
export const GRID = "layout.grid";

export async function motor() {
  resetForTests();
  return createMotor(await readFile(WASM));
}

/** The binary face of a handle's last run, as a `Buffer` so two of them compare directly. */
export function snapshot(motor, handle) {
  motor.layout(handle, GRID);
  return Buffer.from(motor.toBytes(handle));
}

/** What `fn` threw, or `null`. */
export async function caught(fn) {
  try {
    await fn();
    return null;
  } catch (error) {
    return error;
  }
}

/** `u32::MAX` in an optional column: the field is absent. */
export const ABSENT = 0xffff_ffff;

/** The block `assembleColumns` reads: column `c` of row `r` at `c * count + r`, so a column is
 *  its rows contiguous — the contract's structure of arrays. */
export function columnar(columns, count) {
  const out = new Uint32Array(columns.length * count);
  columns.forEach((column, c) => column.forEach((value, r) => { out[c * count + r] = value; }));
  return out;
}

/** Rows for a hand-built document. The node and edge column orders are the contract's, listed
 *  in `docs/contract/ingest-columns.md`; the floats are weights, versions and strengths. */
export function rows(strings, nodeColumns, edgeColumns, floats) {
  return {
    strings,
    nodeCells: columnar(nodeColumns, nodeColumns[0].length),
    edgeCells: columnar(edgeColumns, edgeColumns[0].length),
    weights: Float64Array.from(floats[0]),
    versions: Float64Array.from(floats[1]),
    strengths: Float64Array.from(floats[2]),
  };
}

/** Three nodes, two edges, every optional present or absent on purpose. */
export function asciiRows() {
  return rows(
    ["n-0", "record", "db-0", "studio", "Graph notes", "n-1", "note", "", "n-2", "notes",
      "e-0", "relation", "e-1", "hierarchy"],
    [
      [0, 5, 8],               // id
      [1, 6, 1],               // kind
      [2, ABSENT, 2],          // database_id
      [3, 3, 3],               // source
      [4, 7, 9],               // label
      [ABSENT, ABSENT, ABSENT],// group
      [ABSENT, ABSENT, ABSENT],// icon
      [1, 0, 0],               // has_note
    ],
    [
      [10, 12],                // id
      [0, 1],                  // source row
      [1, 2],                  // target row
      [11, 13],                // kind
      [11, 13],                // label
      [ABSENT, ABSENT],        // record_id
      [0, 1],                  // directed
      [0, 0],                  // child_first
    ],
    [[1, 0.5, 0.25], [0, 0, 0], [0.5, 0.5]],
  );
}

/** The same graph as a `ColumnsDocument`, in the order `encodeColumns` walks it: node fields
 *  row by row, so the interned table lands in the order `asciiRows` spells out. */
export function asciiDocument() {
  const node = (id, kind, database_id, label, weight, has_note) => ({
    id, kind, database_id, source: "studio", label, group: null, weight, version: 0,
    has_note, icon: null,
  });
  const edge = (id, source, target, kind, directed) => ({
    id, source, target, kind, label: kind, strength: 0.5, directed, record_id: null,
    child_first: false,
  });
  return {
    version: 1,
    nodes: [
      node("n-0", "record", "db-0", "Graph notes", 1, true),
      node("n-1", "note", null, "", 0.5, false),
      node("n-2", "record", "db-0", "notes", 0.25, false),
    ],
    edges: [
      edge("e-0", "n-0", "n-1", "relation", false),
      edge("e-1", "n-1", "n-2", "hierarchy", true),
    ],
  };
}

/** The ids the motor holds after a run, in its own dense order. */
export function idsOf(motor, handle) {
  return JSON.parse(motor.toJSON(motor.layout(handle, GRID).handle)).nodes.id;
}
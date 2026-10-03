// `gm_build_columns` over the real release artifact (`harness/sdk-smoke.mjs` is the same
// path: the module is read from `target/wasm32-unknown-unknown/release/graph_wasm.wasm`).
//
// What it pins, in order of how badly a regression would hurt:
//   * the same document through `build` (JSON) and `buildColumns` (binary) gives the same
//     node count and byte-identical snapshot bytes — so the Rust differential is not the
//     only thing standing between the two paths;
//   * a `-0.0` weight is neither refused nor normalised on the way in;
//   * a lone surrogate is refused by the encoder, by field name, rather than becoming U+FFFD
//     and reading back as a different id;
//   * a repeated node id is refused by the motor, as `ColumnsInvalid`, where `build` drops
//     it first-wins — the dense-row rule, seen from the host;
//   * a module without `gm_build_columns` is refused, naming the export it lacks;
//   * `assembleColumns` — the encoder for a producer that already holds columns — produces
//     byte-identical output to `encodeColumns`, takes the ASCII fast path, falls to the exact
//     path for a multi-byte table, and refuses what `encodeColumns` refuses.
//
// Run: `node --test --experimental-strip-types crates/graph-sdk-js/test/`.

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  assembleColumns,
  ColumnsEncoderError,
  createMotor,
  encodeColumns,
  GraphMotorError,
} from "../src/index.ts";
import { loadMotor, resetForTests } from "../src/wasm.ts";

const WASM = fileURLToPath(
  new URL("../../../target/wasm32-unknown-unknown/release/graph_wasm.wasm", import.meta.url),
);

/** A small document with the awkward values: absent optionals, `-0.0`, a subnormal, a
 *  multi-byte id, a `child_first` edge — every field class `docs/contract/ingest-columns.md`
 *  can carry, in five nodes and five edges. */
function document() {
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
const GRID = "layout.grid";

async function motor() {
  resetForTests();
  return createMotor(await readFile(WASM));
}

/** The binary face of a handle's last run, as a `Buffer` so two of them compare directly. */
function snapshot(motor, handle) {
  motor.layout(handle, GRID);
  return Buffer.from(motor.toBytes(handle));
}

/** What `fn` threw, or `null`. */
async function caught(fn) {
  try {
    await fn();
    return null;
  } catch (error) {
    return error;
  }
}

/** `u32::MAX` in an optional column: the field is absent. */
const ABSENT = 0xffff_ffff;

/** The block `assembleColumns` reads: column `c` of row `r` at `c * count + r`, so a column is
 *  its rows contiguous — the contract's structure of arrays. */
function columnar(columns, count) {
  const out = new Uint32Array(columns.length * count);
  columns.forEach((column, c) => column.forEach((value, r) => { out[c * count + r] = value; }));
  return out;
}

/** Rows for a hand-built document. The node and edge column orders are the contract's, listed
 *  in `docs/contract/ingest-columns.md`; the floats are weights, versions and strengths. */
function rows(strings, nodeColumns, edgeColumns, floats) {
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
function asciiRows() {
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
function asciiDocument() {
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
function idsOf(motor, handle) {
  return JSON.parse(motor.toJSON(motor.layout(handle, GRID).handle)).nodes.id;
}

test("the two build paths give the same graph and the same layout bytes", async () => {
  const m = await motor();
  const doc = document();
  const viaJson = m.build(JSON.stringify(doc));
  const viaColumns = m.buildColumns(encodeColumns(doc));
  assert.equal(m.nodeCount(viaColumns), m.nodeCount(viaJson));
  assert.equal(m.nodeCount(viaJson), doc.nodes.length);
  const jsonBytes = snapshot(m, viaJson);
  const columnsBytes = snapshot(m, viaColumns);
  assert.ok(jsonBytes.length > 0, "the layout produced bytes");
  assert.equal(jsonBytes.length, columnsBytes.length, "the two graphs have the same shape");
  assert.ok(jsonBytes.equals(columnsBytes), "identical snapshot bytes");
});

test("a negative zero weight is neither refused nor normalised by the encoder", async () => {
  const m = await motor();
  // A `-0.0` that became `0.0` would still be a *valid* graph, so the check is on the bytes:
  // the two encodings must differ and both documents must build. Whether the sign reaches a
  // layout is the layout's business — `layout.grid` does not read weight at all — so the
  // value itself is pinned by the Rust differential, whose corpus carries a `-0.0` node and
  // compares the whole `Topology`.
  const signed = encodeColumns(document());
  const plain = document();
  plain.nodes[0].weight = 0.0;
  const plainBytes = encodeColumns(plain);
  assert.notDeepEqual(signed, plainBytes, "-0.0 was normalised away by the encoder");
  assert.equal(
    m.nodeCount(m.buildColumns(signed)),
    m.nodeCount(m.buildColumns(plainBytes)),
    "both build: -0.0 is an ordinary finite double",
  );
});

test("a lone surrogate is refused by the encoder, by field", () => {
  const doc = document();
  doc.nodes[2].id = "n-\ud800";
  assert.throws(
    () => encodeColumns(doc),
    (error) => {
      assert.ok(error instanceof ColumnsEncoderError, String(error));
      assert.equal(error.field, "nodes[2].id");
      assert.match(error.message, /not well-formed/);
      return true;
    },
  );
});

test("a document whose endpoint is not a node is refused before it reaches wasm", () => {
  const doc = document();
  doc.edges[0].target = "n-ghost";
  assert.throws(() => encodeColumns(doc), ColumnsEncoderError);
});

test("a repeated node id is refused as ColumnsInvalid, where build drops it", async () => {
  const m = await motor();
  // Appended rather than renamed: a rename would leave the edges that named the old id
  // dangling, and the *encoder* would refuse that first — a different refusal, and not the
  // one this test is about.
  const repeated = document();
  repeated.nodes.push({ ...repeated.nodes[0] });
  const error = await caught(() => m.buildColumns(encodeColumns(repeated)));
  assert.ok(error, "a repeated id is refused");
  assert.equal(error.name, "ColumnsRefusedError");
  assert.equal(error.codeName, "ColumnsInvalid");
  assert.equal(error.code, 23);
});

test("a module without gm_build_columns is refused, naming the export it lacks", async () => {
  // The real module with the export's name scribbled over. The loader names what is missing
  // on `console.error` and hands the caller a `WasmUnavailableError` whose own message is the
  // summary, so the name is asserted where it is actually written — and around the *only*
  // attempt, because the `initFailed` latch would swallow a second one silently.
  const source = await readFile(WASM);
  const trimmed = Buffer.from(source);
  const index = trimmed.indexOf(Buffer.from("gm_build_columns", "utf8"));
  assert.ok(index > 0, "the export name is in the module");
  trimmed.fill(0x5a, index, index + "gm_build_columns".length);
  const reported = [];
  const quiet = console.error;
  console.error = (...args) => reported.push(args.map(String).join(" "));
  resetForTests();
  let error = null;
  try {
    await loadMotor(trimmed);
  } catch (caughtError) {
    error = caughtError;
  } finally {
    console.error = quiet;
  }
  assert.ok(error instanceof Error, "a module missing the export must not load");
  assert.match(String(error.message), /failed to load/);
  const detail = reported.join("\n");
  assert.match(detail, /gm_build_columns/, "the refusal names the missing export");
  assert.match(detail, /older than this SDK/);
  const degraded = await createMotor(trimmed);
  assert.equal(degraded.available, false, "and a Motor built from it is degraded, not working");
});
test("assembleColumns and encodeColumns give the same bytes for the same document", () => {
  const built = assembleColumns(asciiRows());
  const interned = encodeColumns(asciiDocument());
  assert.ok(built.length > 0, "the assembler wrote a document");
  assert.equal(built.length, interned.length, "the same sections, so the same length");
  assert.ok(Buffer.from(built).equals(Buffer.from(interned)), "identical bytes");
});

test("a table with duplicate entries builds, and each row keeps its own id", async () => {
  const m = await motor();
  // "n-0", "n-1" and "e-0" each appear twice: `docs/contract/ingest-columns.md` says two
  // entries may hold the same bytes and the arena interns by content, so the repeats cost
  // nothing downstream. The dense-row rule is the other half — the *ids* stay unique, or the
  // motor refuses the document as ColumnsInvalid.
  const repeated = assembleColumns(rows(
    ["n-0", "record", "db-0", "studio", "Graph notes", "n-0", "n-1", "note", "", "n-2",
      "n-1", "notes", "e-0", "relation", "e-0", "e-1", "hierarchy"],
    [
      [0, 6, 9], [1, 7, 1], [2, ABSENT, 2], [3, 3, 3], [4, 8, 11],
      [ABSENT, ABSENT, ABSENT], [ABSENT, ABSENT, ABSENT], [1, 0, 0],
    ],
    [
      [12, 15], [0, 1], [1, 2], [13, 16], [13, 16], [ABSENT, ABSENT], [0, 1], [0, 0],
    ],
    [[1, 0.5, 0.25], [0, 0, 0], [0.5, 0.5]],
  ));
  const handle = m.buildColumns(repeated);
  assert.equal(m.nodeCount(handle), 3);
  assert.deepEqual(idsOf(m, handle), ["n-0", "n-1", "n-2"]);
});

test("a multi-byte string takes the exact path, builds, and its id reads back the same", async () => {
  const m = await motor();
  // "é" is one code unit and two UTF-8 bytes, which is what makes the ASCII fast path miss:
  // a buffer sized at one byte per code unit cannot hold it.
  const table = ["n-é", "record", "db-0", "studio", "Graph notes", "n-1", "note", "notes é",
    "n-2", "notes", "e-0", "relation", "e-1", "hierarchy"];
  const exact = assembleColumns(rows(
    table,
    [
      [0, 5, 8], [1, 6, 1], [2, ABSENT, 2], [3, 3, 3], [4, 7, 9],
      [ABSENT, ABSENT, ABSENT], [ABSENT, ABSENT, ABSENT], [1, 0, 0],
    ],
    [
      [10, 12], [0, 1], [1, 2], [11, 13], [11, 13], [ABSENT, ABSENT], [0, 1], [0, 0],
    ],
    [[1, 0.5, 0.25], [0, 0, 0], [0.5, 0.5]],
  ));
  // The exact path's offsets are the UTF-8 running sums, not the code-unit counts that sized
  // the fast path's buffer: two é's are four bytes against two code units.
  const blobBytes = table.reduce((sum, value) => sum + Buffer.byteLength(value, "utf8"), 0);
  const declared = new DataView(exact.buffer).getUint32(20, true);
  assert.equal(declared, blobBytes, "the header declares the UTF-8 blob length");
  assert.notEqual(blobBytes, table.join("").length, "the two widths really do differ here");
  const handle = m.buildColumns(exact);
  assert.equal(m.nodeCount(handle), 3);
  const ids = idsOf(m, handle);
  assert.equal(ids[0], "n-é", "the multi-byte id came back as itself");
  assert.equal(ids[1], "n-1");
  assert.equal(ids[2], "n-2");
  assert.ok(!ids.some((id) => id.includes("�")), "nothing became U+FFFD on the way through");
});

test("a lone surrogate in the table is refused, naming its index", () => {
  const broken = asciiRows();
  const table = [...broken.strings];
  table[0] = "n-\ud800";
  assert.throws(
    () => assembleColumns({ ...broken, strings: table }),
    (error) => {
      assert.ok(error instanceof ColumnsEncoderError, String(error));
      assert.equal(error.field, "strings[0]");
      assert.match(error.message, /not well-formed/);
      return true;
    },
  );
});

test("two lone surrogates that join into a valid pair are still refused", () => {
  const broken = asciiRows();
  // Joined, "\ud800" + "\udc00" is U+10000: one well-formed code point, so a check on the
  // joined text alone would pass and both entries would read back as U+FFFD. The check is
  // per string for exactly this reason.
  const table = [...broken.strings];
  table[0] = "\ud800";
  table[5] = "\udc00";
  assert.throws(
    () => assembleColumns({ ...broken, strings: table }),
    (error) => {
      assert.ok(error instanceof ColumnsEncoderError, String(error));
      assert.equal(error.field, "strings[0]");
      return true;
    },
  );
});

test("a column of the wrong length is refused, naming the array", () => {
  const built = asciiRows();
  const short = { ...built, nodeCells: new Uint32Array(built.nodeCells.length - 1) };
  assert.throws(
    () => assembleColumns(short),
    (error) => {
      assert.ok(error instanceof GraphMotorError, String(error));
      assert.match(error.message, /nodeCells/);
      return true;
    },
  );
  const mismatched = { ...built, versions: new Float64Array(built.versions.length + 1) };
  assert.throws(
    () => assembleColumns(mismatched),
    (error) => {
      assert.ok(error instanceof GraphMotorError, String(error));
      assert.match(error.message, /versions/);
      return true;
    },
  );
  assert.doesNotThrow(() => assembleColumns(built), "the lengths that do match still build");
});

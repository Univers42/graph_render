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
//   * a module without `gm_build_columns` is refused, naming the export it lacks.
//
// Run: `node --test --experimental-strip-types crates/graph-sdk-js/test/`.

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { createMotor, encodeColumns, ColumnsEncoderError } from "../src/index.ts";
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
  assert.equal(error.code, 20);
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
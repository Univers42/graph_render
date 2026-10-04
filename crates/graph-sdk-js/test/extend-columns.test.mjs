// `Motor.extendColumns` and `encodeBatch` against a real module: the GMX1 batch
// `gm_graph_extend_columns` reads (`docs/decisions/extend-columns.md` conditions 13-16, and
// `docs/contract/delta.md` "The SDK"). Written before the code, and observed red.
//
// What it pins, in order of how badly a regression would hurt:
//   * the same batch through `extend` (JSON) and `extendColumns` (GMX1) leaves the same graph
//     — one `u32` error code apart (`IngestInvalid` there, `ColumnsInvalid` here), so the bytes
//     are the only thing that can tell the two paths apart;
//   * a view taken before an `extendColumns` is refused afterwards, and a fresh run reads the
//     grown graph — the bump is real, not inherited;
//   * every refusal is `ColumnsRefusedError` with `ColumnsInvalid`, and a released graph is
//     `InvalidHandleError`, so U1's cost cannot be paid by accident;
//   * an edge may name a node the graph already holds — the one thing a batch may do that a
//     whole document's dense-row endpoint cannot;
//   * a lone surrogate is refused by the encoder, by field name, as `encodeColumns` refuses it;
//   * a module without `gm_graph_extend_columns` is refused, naming the export it lacks.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/
//
// Needs a built module at target/wasm32-unknown-unknown/release/graph_wasm.wasm, as
// `delta.test.mjs` does.

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { after, before, test } from "node:test";

import {
  ColumnsEncoderError,
  ColumnsRefusedError,
  ColumnId,
  GraphMotorError,
  InvalidHandleError,
  Motor,
  encodeBatch,
  encodeColumns,
} from "../src/index.ts";
import { loadMotor, resetForTests } from "../src/wasm.ts";

import { WASM, caught, snapshot } from "./columns-fixtures.mjs";

const INVALID_HANDLE = 1;
const COLUMNS_INVALID = 23;

const node = (id) => ({
  id, kind: "record", database_id: null, source: "s", label: id.toUpperCase(),
  group: null, weight: 0.5, version: 0, has_note: false, icon: null,
});
const edge = (id, source, target) => ({
  id, source, target, kind: "relation", label: "", strength: 0.5, directed: false, record_id: null,
  child_first: false,
});

/** Four nodes on a path: the graph every test grows. */
const BASE = {
  version: 1,
  nodes: ["a", "b", "c", "d"].map(node),
  edges: [edge("e0", "a", "b"), edge("e1", "b", "c"), edge("e2", "c", "d")],
};
/** Two new nodes: one edge onto a node the graph already holds, one between the two. */
const BATCH = {
  nodes: [node("x"), node("y")],
  edges: [edge("e3", "d", "x"), edge("e4", "x", "y")],
};

/** What `run` threw, or `null`. */
function refusalOf(run) {
  try {
    run();
    return null;
  } catch (error) {
    return error;
  }
}

function assertRefused(run, ErrorClass, code, why) {
  const error = refusalOf(run);
  assert.ok(error instanceof ErrorClass, `${why}: expected ${ErrorClass.name}, got ${String(error)}`);
  assert.equal(error.code, code, why);
}

let motor;
before(async () => {
  resetForTests();
  motor = await Motor.create(await readFile(WASM));
  assert.ok(motor.available, "the module did not load; every test below would be vacuous");
});
after(() => resetForTests());

test("the two extend paths append the same batch to the same graph", () => {
  const viaJson = motor.build(JSON.stringify(BASE));
  const viaColumns = motor.buildColumns(encodeColumns(BASE));
  motor.extend(viaJson, BATCH);
  motor.extendColumns(viaColumns, BATCH);
  assert.equal(motor.nodeCount(viaJson), 6);
  assert.equal(motor.nodeCount(viaColumns), 6);
  assert.deepEqual(snapshot(motor, viaJson), snapshot(motor, viaColumns), "the same layout bytes");
  motor.release(viaJson);
  motor.release(viaColumns);
});

test("a view taken before extendColumns is refused after it, and a fresh run reads the batch", () => {
  const graph = motor.build(JSON.stringify(BASE));
  motor.layout(graph, "layout.grid");
  const before = motor.column(graph, ColumnId.NodeX);
  const epoch = motor.epoch;
  motor.extendColumns(graph, BATCH);

  assert.ok(motor.epoch > epoch, "the epoch moved, so no cached view is served again");
  assertRefused(
    () => motor.column(graph, ColumnId.NodeX),
    InvalidHandleError,
    undefined,
    "the run was cleared, so a stale view is refused rather than served",
  );
  motor.layout(graph, "layout.grid");
  const after = motor.column(graph, ColumnId.NodeX);
  assert.equal(before.length, 4);
  assert.equal(after.length, 6, "the new run reads the grown graph");
  motor.release(graph);
});

test("each extendColumns refusal is ColumnsRefusedError, and the graph is unchanged", async () => {
  const graph = motor.build(JSON.stringify(BASE));
  const taken = { nodes: [node("a")], edges: [] };
  const dangling = { nodes: [], edges: [edge("e9", "a", "nowhere")] };
  assertRefused(() => motor.extendColumns(graph, taken), ColumnsRefusedError, COLUMNS_INVALID, "a taken id");
  assertRefused(() => motor.extendColumns(graph, dangling), ColumnsRefusedError, COLUMNS_INVALID, "a dangling endpoint");
  const error = await caught(() => motor.extendColumns(graph, encodeBatch({})));
  assert.ok(error instanceof ColumnsRefusedError, `no batch at all: got ${String(error)}`);
  assert.equal(error.codeName, "ColumnsInvalid");
  assert.equal(motor.nodeCount(graph), 4, "a refused batch claimed no id");
  motor.extendColumns(graph, BATCH);
  assert.equal(motor.nodeCount(graph), 6);

  motor.release(graph);
  assertRefused(() => motor.extendColumns(graph, BATCH), InvalidHandleError, INVALID_HANDLE, "a released graph");
});

test("each reader refuses the other format's bytes, by class", async () => {
  const graph = motor.build(JSON.stringify(BASE));
  // A whole document's `GMC1` at the batch export: dense rows are not string indices, and the
  // magic is what says so.
  const wrong = await caught(() => motor.extendColumns(graph, encodeColumns(BASE)));
  assert.ok(wrong instanceof ColumnsRefusedError, `GMC1 at the batch export: got ${String(wrong)}`);
  assert.equal(wrong.code, COLUMNS_INVALID);
  // And a `GMX1` batch at the document build, which is the other direction of the same rule.
  const other = await caught(() => motor.buildColumns(encodeBatch(BATCH)));
  assert.ok(other instanceof ColumnsRefusedError, `GMX1 at the document build: got ${String(other)}`);
  assert.equal(other.code, COLUMNS_INVALID);
  assert.equal(motor.nodeCount(graph), 4);
  motor.release(graph);
});

test("a batch edge may name a node the graph already holds, and one a node of its own", () => {
  const graph = motor.build(JSON.stringify(BASE));
  motor.extendColumns(graph, BATCH);
  assert.equal(motor.nodeCount(graph), 6, "`e3` names `d`, which this batch does not carry");
  motor.release(graph);
});

test("a lone surrogate is refused by encodeBatch, by field", () => {
  const batch = { nodes: [node("x")], edges: [] };
  batch.nodes[0].label = "l\ud800b";
  assert.throws(
    () => encodeBatch(batch),
    (error) => {
      assert.ok(error instanceof ColumnsEncoderError, String(error));
      assert.equal(error.field, "nodes[0].label");
      assert.match(error.message, /not well-formed/);
      return true;
    },
  );
  batch.nodes[0].label = "l";
  const dangling = { nodes: [], edges: [edge("e9", "a", "no\ud800where")] };
  assert.throws(
    () => encodeBatch(dangling),
    (error) => {
      assert.ok(error instanceof ColumnsEncoderError, String(error));
      assert.equal(error.field, "edges[0].target");
      return true;
    },
  );
});

test("a module without gm_graph_extend_columns is refused, naming the export it lacks", async () => {
  // The real module with the export's name scribbled over — the same probe
  // `columns-build.test.mjs` runs for `gm_build_columns`, on the name this SDK adds. The loader
  // names what is missing on `console.error`, and around the *only* attempt, because the
  // `initFailed` latch would swallow a second one silently.
  const name = "gm_graph_extend_columns";
  const trimmed = Buffer.from(await readFile(WASM));
  const index = trimmed.indexOf(Buffer.from(name, "utf8"));
  assert.ok(index > 0, "the export name is in the module");
  trimmed.fill(0x5a, index, index + name.length);
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
  const detail = reported.join("\n");
  assert.match(detail, new RegExp(name), "the refusal names the missing export");
  assert.match(detail, /older than this SDK/);
  const degraded = await Motor.create(trimmed);
  assert.equal(degraded.available, false, "and a Motor built from it is degraded, not working");
  assert.throws(() => degraded.extendColumns(1, BATCH), (thrown) => {
    assert.ok(thrown instanceof GraphMotorError, String(thrown));
    return true;
  }, "and every method on it refuses predictably");
});

// `Motor.extend` and `ForceSession.grow` against a real module (`docs/contract/delta.md` "The
// SDK"): a grown session moves the rows a batch appended, a view taken before a batch is stale
// after it, and each refusal is the typed error its code names, with the graph or the session
// left as it was.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/
//
// Needs a built module at target/wasm32-unknown-unknown/release/graph_wasm.wasm, as
// `motor.test.mjs` does.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import {
  BuildRefusedError,
  ColumnId,
  ForceSessionRefusedError,
  InvalidHandleError,
  InvalidSessionError,
  Motor,
  resetForTests,
} from "../src/index.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const WASM = join(HERE, "..", "..", "..", "target", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");

if (!existsSync(WASM)) {
  throw new Error(`${WASM} is missing: build it with scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`);
}

const INVALID_HANDLE = 1;
const INGEST_INVALID = 4;
const INVALID_SESSION = 15;
const SESSION_REFUSED = 17;

const node = (id) => ({
  id, kind: "record", database_id: null, source: "s", label: id.toUpperCase(),
  group: null, weight: 0.5, version: 0, has_note: false, icon: null,
});
const edge = (id, source, target) => ({
  id, source, target, kind: "relation", label: "", strength: 0.5, directed: false, record_id: null,
  child_first: false,
});

/** Four nodes on a path: the graph every test grows. */
const BASE = JSON.stringify({
  version: 1,
  nodes: ["a", "b", "c", "d"].map(node),
  edges: [edge("e0", "a", "b"), edge("e1", "b", "c"), edge("e2", "c", "d")],
});
/** Two new nodes, an edge from each to an old node and one between them. */
const BATCH = {
  nodes: [node("x"), node("y")],
  edges: [edge("e3", "d", "x"), edge("e4", "x", "y"), edge("e5", "a", "y")],
};

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

const copyOf = (session) => {
  const { xs, ys } = session.positions();
  return { xs: Array.from(xs), ys: Array.from(ys) };
};

let motor;
before(async () => {
  motor = await Motor.create(readFileSync(WASM));
  assert.ok(motor.available, "the module did not load; every test below would be vacuous");
});
after(() => resetForTests());

test("grow after extend takes the session onto the new rows, and the ticks move them", () => {
  const graph = motor.build(BASE);
  const session = motor.forceSession(graph);
  session.tick(10);
  const old = copyOf(session);
  motor.extend(graph, BATCH);
  assert.equal(motor.nodeCount(graph), 6);
  assert.equal(session.positions().xs.length, 4, "the session sees the batch only after grow");

  session.grow(graph);
  const grown = copyOf(session);
  assert.equal(grown.xs.length, 6);
  assert.deepEqual(grown.xs.slice(0, 4), old.xs, "grow moves no old row");
  assert.ok([...grown.xs, ...grown.ys].every(Number.isFinite), "the new rows are placed");

  session.tick(10);
  const ticked = copyOf(session);
  for (const row of [4, 5]) {
    assert.notDeepEqual([ticked.xs[row], ticked.ys[row]], [grown.xs[row], grown.ys[row]], `row ${row} moved`);
  }
  session.release();
  motor.release(graph);
});

test("a column view taken before extend is stale after it, and the run is cleared", () => {
  const graph = motor.build(BASE);
  motor.layout(graph, "layout.grid");
  const before = motor.column(graph, ColumnId.NodeX);
  const epoch = motor.epoch;
  motor.extend(graph, BATCH);

  assert.ok(motor.epoch > epoch, "the epoch moved, so no cached view is served again");
  assertRefused(() => motor.column(graph, ColumnId.NodeX), InvalidHandleError, undefined, "the run was cleared");
  motor.layout(graph, "layout.grid");
  const after = motor.column(graph, ColumnId.NodeX);
  assert.equal(before.length, 4);
  assert.equal(after.length, 6, "the new run reads the grown graph");
  motor.release(graph);
});

test("a session view taken before grow is stale after it", () => {
  const graph = motor.build(BASE);
  const session = motor.forceSession(graph);
  const before = session.positions().xs;
  motor.extend(graph, BATCH);
  session.grow(graph);
  const after = session.positions().xs;
  assert.notEqual(after, before, "grow re-derives the view");
  assert.equal(after.length, 6);
  session.release();
  motor.release(graph);
});

test("each extend refusal is its typed error, and the graph is unchanged", () => {
  const graph = motor.build(BASE);
  const taken = { nodes: [node("a")], edges: [] };
  const dangling = { nodes: [], edges: [edge("e9", "a", "nowhere")] };
  assertRefused(() => motor.extend(graph, taken), BuildRefusedError, INGEST_INVALID, "a taken id");
  assertRefused(() => motor.extend(graph, dangling), BuildRefusedError, INGEST_INVALID, "a dangling endpoint");
  assertRefused(() => motor.extend(graph, undefined), BuildRefusedError, undefined, "no batch at all");
  assertRefused(() => motor.extend(graph, { nodes: [] }), BuildRefusedError, undefined, "no edges member");
  assert.equal(motor.nodeCount(graph), 4);
  motor.extend(graph, BATCH);
  assert.equal(motor.nodeCount(graph), 6, "a refused batch claimed no id");

  motor.release(graph);
  assertRefused(() => motor.extend(graph, BATCH), InvalidHandleError, INVALID_HANDLE, "a released graph");
});

test("each grow refusal is its typed error, and the session is unchanged", () => {
  const graph = motor.build(BASE);
  const other = motor.build(BASE);
  const session = motor.forceSession(graph);
  session.tick(3);
  const before = copyOf(session);
  motor.extend(other, BATCH);
  assertRefused(() => session.grow(other), ForceSessionRefusedError, SESSION_REFUSED, "another graph");
  assert.deepEqual(copyOf(session), before);

  motor.release(graph);
  assertRefused(() => session.grow(graph), InvalidHandleError, INVALID_HANDLE, "a released graph");
  assert.deepEqual(copyOf(session), before);

  session.release();
  assertRefused(() => session.grow(other), InvalidSessionError, INVALID_SESSION, "a released session");
  motor.release(other);
});

test("a grow with nothing appended succeeds and moves nothing", () => {
  const graph = motor.build(BASE);
  const session = motor.forceSession(graph);
  session.tick(3);
  const before = copyOf(session);
  session.grow(graph);
  assert.deepEqual(copyOf(session), before);
  session.release();
  motor.release(graph);
});

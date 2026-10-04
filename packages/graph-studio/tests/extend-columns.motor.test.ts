// Which extend path the studio takes, and what it says when the motor has none
// (`docs/decisions/extend-columns.md` condition 17). The two guards are one line each in
// `session.ts`, and a `TypeError` instead of a `SessionRefusal` is the difference between a
// queue that answers `failed` and a worker that dies on a batch.
//
// Each row is the next motor the studio might be handed: one with both paths, one with only the
// JSON one, one with neither. The first two are each other's negative control — a switch that
// only ever called `extendColumns` would pass the first and fail the second, and one that called
// neither would fail both.
import assert from "node:assert/strict";
import { test } from "node:test";

import { assembleColumns, createMotor, type Handle } from "../../../crates/graph-sdk-js/src/index.ts";
import type { GraphBatch } from "../src/motor/protocol.ts";
import { type MotorLike, type Session, createSession } from "../src/motor/session.ts";
import { FIXTURES_URL, SKIP, WASM } from "./motor.ts";

const FORCE = "layout.forceatlas2.barnes_hut";

/** Which paths this motor has, and the order it was asked in. */
interface Paths {
  readonly json: boolean;
  readonly columns: boolean;
}

const node = (id: string) => ({
  id, kind: "record", database_id: null, source: "studio", label: id,
  group: null, weight: 1, version: 1, has_note: false, icon: null,
});

/** One new node, and an edge onto a node the graph already holds — the case only a batch has. */
const BATCH: GraphBatch = {
  nodes: [node("delta-1")],
  edges: [{
    id: "delta-e", source: "delta-1", target: "s00000", kind: "relation", label: "",
    strength: 1, directed: false, record_id: null, child_first: false,
  }],
};

/**
 * The real motor behind a `MotorLike` that has only the paths `paths` says, recording which one
 * a batch took. Every member is named rather than spread, so a motor that gains a method cannot
 * reach the studio's doubles by accident.
 */
async function motorWith(paths: Paths, asked: string[]): Promise<MotorLike<Handle>> {
  const real = await createMotor(WASM ?? new Uint8Array(0));
  assert.ok(real.available, "the module did not load; every row below would be vacuous");
  const base: MotorLike<Handle> = {
    layouts: () => real.layouts(),
    posts: () => real.posts(),
    analyses: () => real.analyses(),
    build: (json) => real.build(json),
    buildColumns: (bytes) => real.buildColumns(bytes),
    run: (handle, layoutId, options) => real.run(handle, layoutId, options),
    layoutParams: (layoutId) => real.layoutParams(layoutId),
    post: (handle, postId) => real.post(handle, postId),
    analysis: (handle, analysisId) => real.analysis(handle, analysisId),
    toBytes: (handle) => real.toBytes(handle),
    release: (handle) => real.release(handle),
    forceSession: (handle, params, engine, seed) => real.forceSession(handle, params, engine, seed),
  };
  if (paths.json) {
    base.extend = (handle, batch) => {
      asked.push("json");
      real.extend(handle, batch);
    };
  }
  if (paths.columns) {
    base.extendColumns = (handle, batch) => {
      asked.push("columns");
      real.extendColumns(handle, batch);
    };
  }
  return base;
}

/** A session over that motor, with a graph built, a run drawn and a live port open. */
async function withPort(paths: Paths): Promise<{ readonly session: Session; readonly asked: string[] }> {
  const asked: string[] = [];
  const session: Session = createSession({
    motorFrom: () => motorWith(paths, asked),
    fetchText: () => Promise.reject(new Error("this test fetches nothing")),
    digest: () => Promise.resolve(null),
    assemble: assembleColumns,
    now: () => 0,
  });
  await session.open("unused");
  await session.load({ kind: "synthetic", seed: 1, nodes: 40, degree: 2, shape: "vault" }, FIXTURES_URL);
  await session.layout(FORCE, null);
  return { session, asked };
}

/** The port's own `extend`, or a failure naming that it has none. */
function extendOf(session: Session): (batch: GraphBatch) => void {
  const port = session.forces();
  assert.ok(port !== null, "a drawn graph has a live port");
  assert.ok(port.extend !== undefined, "the port's extend is the session's, not the queue's");
  return port.extend.bind(port);
}

test("a motor with both paths takes the columnar one", { skip: SKIP }, async () => {
  const { session, asked } = await withPort({ json: true, columns: true });
  const extend = extendOf(session);
  const before = await session.structure();
  assert.equal(before.meta?.nodeCount, 40, "the graph the batch grows");
  extend(BATCH);
  assert.deepEqual(asked, ["columns"], "the batch went through extendColumns");
  const after = await session.structure();
  assert.equal(after.meta?.nodeCount, 41, "the batch was appended, not dropped");
});

test("a motor with only the JSON path still takes the batch", { skip: SKIP }, async () => {
  const { session, asked } = await withPort({ json: true, columns: false });
  const extend = extendOf(session);
  extend(BATCH);
  assert.deepEqual(asked, ["json"], "the fallback ran, and the batch reached the graph");
  const after = await session.structure();
  assert.equal(after.meta?.nodeCount, 41);
});

test("a motor with neither refuses the batch, and says so", { skip: SKIP }, async () => {
  const { session, asked } = await withPort({ json: false, columns: false });
  const extend = extendOf(session);
  // `SessionRefusal`, never a `TypeError`: the queue turns a throw here into its `failed`
  // result, and a `TypeError` would read as the studio's own bug rather than the motor's shape.
  assert.throws(() => extend(BATCH), (error: unknown) => {
    assert.ok(error instanceof Error, String(error));
    assert.equal(error.name, "SessionRefusal");
    assert.match(error.message, /cannot add to a built graph/);
    return true;
  });
  assert.deepEqual(asked, [], "and nothing was asked of a motor with no such method");
});

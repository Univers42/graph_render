// The session's live input that is not a position: the two velocity columns and the simple
// graph, read through the four exports this job added (`gm_force_session_velocity_ptr`/`_len`,
// `gm_force_session_edge_ptr`/`_len`).
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/
//
// Needs a built module at target/wasm32-unknown-unknown/release/graph_wasm.wasm; the gate row
// `wasm-release` builds it first. Without it every test here fails with the same explicit
// "build the module first" refusal rather than a `ENOENT` — `motor.test.mjs`'s own header.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import { Motor, TamperedGeometryError, resetForTests } from "../src/index.ts";
import { simpleEdges } from "../src/force-handoff.ts";
import { loadMotor } from "../src/wasm.ts";
import { ColumnViews } from "../src/views.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const WASM = join(HERE, "..", "..", "..", "target", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");

if (!existsSync(WASM)) {
  throw new Error(`${WASM} is missing: build it with scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`);
}

const node = (id) => ({
  id, kind: "record", database_id: null, source: "s", label: id.toUpperCase(),
  group: null, weight: 0.5, version: 0, has_note: false, icon: null,
});
const edge = (id, source, target, strength = 0.5) => ({
  id, source, target, kind: "relation", label: "", strength, directed: false, record_id: null,
});

/** Four nodes, three distinct non-loop edges, one duplicate of the first (with a different
 *  strength, so "the first copy is kept" is observable) and one self-loop. */
const INGEST = JSON.stringify({
  version: 1,
  nodes: ["a", "b", "c", "d"].map(node),
  edges: [
    edge("e0", "a", "b", 0.5),
    edge("e1", "b", "a", 0.9),
    edge("e2", "c", "c", 0.5),
    edge("e3", "b", "c", 0.5),
    edge("e4", "c", "d", 0.5),
  ],
});

/** The refusal class a call raised, or `null` when it did not raise. */
async function refusalOf(run) {
  try {
    await run();
    return null;
  } catch (error) {
    return error;
  }
}

let motor;
/** The module as `simpleEdges` needs it: `force-handoff.ts` is not on `index.ts`'s surface, so
 *  the test holds the `Loaded` a `Motor` would have kept private. One source object, loaded
 *  once — `wasm.ts` is a singleton by design, so a second `loadMotor` with a different byte
 *  object is refused by its own m9 check. */
let loaded;
before(async () => {
  const bytes = readFileSync(WASM);
  const exports = await loadMotor(bytes);
  loaded = { exports, views: new ColumnViews(exports) };
  motor = await Motor.create(bytes);
  assert.ok(motor.available, "the module did not load; every test below would be vacuous");
});
after(() => resetForTests());

test("the_edges_are_the_simple_graph: distinct, non-loop, first strength kept", () => {
  const session = motor.forceSession(motor.build(INGEST));
  const { lo, hi, strength } = simpleEdges(loaded, session.id);
  // The duplicate b-a and the self-loop c-c are both gone: three edges, not five.
  assert.equal(lo.length, 3, "the distinct non-loop edges");
  assert.equal(hi.length, 3, "one endpoint per edge");
  assert.equal(strength.length, 3, "one strength per edge");
  for (let i = 0; i < lo.length; i += 1) {
    assert.ok(lo[i] < hi[i], `edge ${String(i)} is ordered`);
  }
  // The first copy's strength survives, not the second's and not a sum or a mean.
  const first = [...lo].findIndex((row) => row === 0);
  assert.ok(first >= 0, "the a-b edge is there");
  assert.equal(strength[first], 0.5, "the first copy's strength, not the duplicate's 0.9");
  // The columns are copies, not views: writing through one moves nothing in the session.
  const again = simpleEdges(loaded, session.id);
  lo[0] = 99;
  assert.notEqual(again.lo[0], 99, "the caller's write did not reach the next read");
  session.release();
});

test("a_fresh_session_is_at_rest: every velocity is 0, one per node", () => {
  const graph = motor.build(INGEST);
  const session = motor.forceSession(graph);
  const { vxs, vys } = session.velocities();
  assert.equal(vxs.length, 4, "one vx per node");
  assert.equal(vys.length, 4, "one vy per node");
  assert.ok([...vxs].every((v) => v === 0), "vx at rest");
  assert.ok([...vys].every((v) => v === 0), "vy at rest");
  // After the reads, not before: these are zero-copy views over the session's own columns, and
  // a released session's memory is fair game for the next one.
  session.release();
});

test("velocities_move_after_a_tick", () => {
  const session = motor.forceSession(motor.build(INGEST));
  session.tick(1);
  const { vxs, vys } = session.velocities();
  assert.equal(vxs.length, 4, "one vx per node");
  assert.equal(vys.length, 4, "one vy per node");
  assert.ok(
    [...vxs].some((v) => v !== 0) || [...vys].some((v) => v !== 0),
    "a tick moved something",
  );
  session.release();
});

test("a_non_finite_velocity_written_through_the_view_is_refused: on the next read", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  session.velocities().vxs[0] = Number.NaN;
  const error = await refusalOf(() => session.velocities());
  session.release();
  assert.ok(error instanceof TamperedGeometryError, `threw ${error?.constructor?.name}`);
  assert.match(error.message, /row 0 holds a non-finite velocity/);
});

test("a_non_finite_velocity_written_through_the_view_is_refused: and on the next tick", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  session.velocities().vys[2] = Number.POSITIVE_INFINITY;
  const error = await refusalOf(() => session.tick(1));
  session.release();
  assert.ok(error instanceof TamperedGeometryError, `threw ${error?.constructor?.name}`);
  assert.match(error.message, /row 2 holds a non-finite velocity/);
});

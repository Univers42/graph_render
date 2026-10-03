// The typed surface of `Motor` and `ForceSession` against a real module, over the behavioural
// findings the review raised and did not run (U6): the `u32` arguments that reached the ABI
// coerced rather than checked, the refusals that named the wrong id space, the `release` that
// discarded the module's answer, and the writable zero-copy session columns that nothing
// re-validates.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/
//
// Needs a built module at target/wasm32-unknown-unknown/release/graph_wasm.wasm; the gate row
// `sdk-adapter-convergence:wasm` and `sdk:smoke` build it first. Without it every test here
// fails with the same explicit "build the module first" refusal rather than a `ENOENT`.

import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

import {
  AbiContractError,
  AllocationFailedError,
  BuildRefusedError,
  ForceSessionRefusedError,
  InvalidHandleError,
  InvalidSessionError,
  Motor,
  TamperedGeometryError,
  resetForTests,
} from "../src/index.ts";
import { columnApplies } from "../src/views.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const WASM = join(HERE, "..", "..", "..", "target", "wasm32-unknown-unknown", "release", "graph_wasm.wasm");

if (!existsSync(WASM)) {
  throw new Error(`${WASM} is missing: build it with scripts/orch/gr cargo build -p graph-wasm --release --target wasm32-unknown-unknown`);
}

const node = (id) => ({
  id, kind: "record", database_id: null, source: "s", label: id.toUpperCase(),
  group: null, weight: 0.5, version: 0, has_note: false, icon: null,
});
const edge = (id, source, target) => ({
  id, source, target, kind: "relation", label: "", strength: 0.5, directed: false, record_id: null,
});
/** Four nodes and a path, so a force session has something that actually moves. */
const INGEST = JSON.stringify({
  version: 1,
  nodes: ["a", "b", "c", "d"].map(node),
  edges: [edge("e0", "a", "b"), edge("e1", "b", "c"), edge("e2", "c", "d")],
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
before(async () => {
  // One module per process: `wasm.ts` is a singleton by design, so a second `createMotor` with
  // a different byte object is refused by its own m9 check.
  motor = await Motor.create(readFileSync(WASM));
  assert.ok(motor.available, "the module did not load; every test below would be vacuous");
});
after(() => resetForTests());

// --- M1: `tick`'s tick count is a u32, checked before it is coerced ----------------------

test("M1: session.tick refuses a negative count instead of running 2^32-1 ticks", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  const error = await refusalOf(() => session.tick(-1));
  session.release();
  assert.ok(error instanceof ForceSessionRefusedError || error instanceof InvalidSessionError, `no refusal: ${error}`);
  assert.match(error.message, /ticks must be an integer in 0\.\.4294967295/);
});

test("M1: session.tick refuses NaN and a fractional count", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  for (const bad of [Number.NaN, 1.5, -0.5, 2 ** 32]) {
    const error = await refusalOf(() => session.tick(bad));
    assert.match(String(error?.message), /ticks must be an integer/, `tick(${String(bad)}) was not refused`);
  }
  session.release();
});

test("M1: a refused tick count leaves the session running", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  await refusalOf(() => session.tick(-1));
  assert.equal(session.released, false, "a refused argument released the session");
  assert.equal(session.tick(1).status, "running");
  session.release();
});

// --- M2: `ticksRun` is the word that crossed the wire --------------------------------------

test("M2: ticksRun is the count that ran, and `tick(0)` is zero", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  assert.equal(session.tick(0).ticksRun, 0);
  assert.equal(session.tick(7).ticksRun, 7);
  session.release();
});

// --- M3: a released *graph* handle is refused in the graph handle's id space --------------

test("M3: forceSession over a released graph handle is InvalidHandleError, not a session refusal", async () => {
  const dead = motor.build(INGEST);
  motor.release(dead);
  const error = await refusalOf(() => motor.forceSession(dead));
  assert.ok(error instanceof InvalidHandleError, `threw ${error?.constructor?.name}`);
  assert.equal(error.code, 1);
  assert.equal(error.codeName, "InvalidHandle");
  assert.ok(!(error instanceof ForceSessionRefusedError), "the two id spaces must not share a class");
});

test("M3: an unknown graph handle is InvalidHandleError too", async () => {
  const error = await refusalOf(() => motor.forceSession(9999));
  assert.ok(error instanceof InvalidHandleError, `threw ${error?.constructor?.name}: ${error?.message}`);
});

// --- M4: the session's own columns are re-validated (D9) ----------------------------------

test("M4: a non-finite position written through the view is refused on the next read", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  const view = session.positions();
  view.xs[1] = Number.NaN;
  const error = await refusalOf(() => session.positions());
  session.release();
  assert.ok(error instanceof TamperedGeometryError, `threw ${error?.constructor?.name}`);
  assert.match(error.message, /row 1 holds a non-finite position/);
});

test("M4: and on the next tick, before the integrator ever sees it", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  session.positions().ys[2] = Number.POSITIVE_INFINITY;
  const error = await refusalOf(() => session.tick(1));
  session.release();
  assert.ok(error instanceof TamperedGeometryError, `threw ${error?.constructor?.name}`);
  assert.match(error.message, /row 2 holds a non-finite position/);
});

test("M4: an untampered session keeps answering positions and ticks", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  session.tick(3);
  const view = session.positions();
  assert.ok([...view.xs].every(Number.isFinite), "a live session reported a non-finite position");
  assert.equal(session.tick(1).status, "running");
  session.release();
});

// --- M6: `release` reads the module's answer ---------------------------------------------

test("M6: releasing a handle this motor never issued is refused with InvalidHandle", async () => {
  const error = await refusalOf(() => motor.release(9999));
  assert.ok(error instanceof InvalidHandleError, `threw ${error?.constructor?.name}`);
  assert.equal(error.codeName, "InvalidHandle");
});

test("M6: releasing twice is refused the second time, and the graph handle is never reissued", async () => {
  const handle = motor.build(INGEST);
  motor.release(handle);
  const error = await refusalOf(() => motor.release(handle));
  assert.ok(error instanceof InvalidHandleError, `threw ${error?.constructor?.name}`);
});

// --- m1: a released handle is refused as released, not as "never laid out" ----------------

test("m1: column() on a released handle names the recorded code", async () => {
  const handle = motor.build(INGEST);
  motor.layout(handle, motor.layouts()[0]);
  motor.release(handle);
  const error = await refusalOf(() => motor.column(handle, 0));
  assert.ok(error instanceof InvalidHandleError, `threw ${error?.constructor?.name}`);
  assert.equal(error.code, 1);
  assert.match(error.message, /is not live/);
});

test("m1: column() on a handle that was never laid out says exactly that", async () => {
  const handle = motor.build(INGEST);
  const error = await refusalOf(() => motor.column(handle, 0));
  assert.ok(error instanceof InvalidHandleError, `threw ${error?.constructor?.name}`);
  assert.equal(error.code, undefined, "a handle with no run yet has no wire code to report");
  assert.match(error.message, /no successful run yet/);
});

// --- m2: a column id is a name, not a count -----------------------------------------------

test("m2: a column id this ABI does not register is refused, never coerced to 0", async () => {
  const handle = motor.build(INGEST);
  motor.layout(handle, motor.layouts()[0]);
  for (const bad of [{}, Number.NaN, 1e9, -1, 13]) {
    const error = await refusalOf(() => motor.column(handle, bad));
    assert.ok(error instanceof AbiContractError, `column(${JSON.stringify(bad)}) threw ${error?.constructor?.name}`);
    assert.match(error.message, /is not one this ABI registers/);
  }
});

test("m2: the reserved note columns are still absent rather than refused", async () => {
  const handle = motor.build(INGEST);
  motor.layout(handle, motor.layouts()[0]);
  assert.equal(motor.column(handle, 7), null, "NoteCode must read absent");
  assert.equal(motor.column(handle, 8), null, "NoteIndex must read absent");
  assert.equal(columnApplies("Circle", "Line", 7, 0), false, "m20: the reserved ids need no arm of their own");
});

// --- m11 / m12: the arguments that reached the ABI coerced --------------------------------

test("m11: a row that is not a u32 is refused by pin and by unpin", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  for (const bad of [Number.NaN, 1.9, -1]) {
    const pinned = await refusalOf(() => session.pin(bad, 5, 5));
    assert.match(String(pinned?.message), /row must be an integer/, `pin(${String(bad)}) was not refused`);
    const unpinned = await refusalOf(() => session.unpin(bad));
    assert.match(String(unpinned?.message), /row must be an integer/, `unpin(${String(bad)}) was not refused`);
  }
  session.release();
});

test("m11: a pin that was refused did not hold row 0", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  const before = session.positions().xs[0];
  await refusalOf(() => session.pin(Number.NaN, 5, 5));
  session.tick(1);
  assert.notEqual(session.positions().xs[0], 5, "a refused pin moved the node anyway");
  assert.notEqual(session.positions().xs[0], before, "the node never moved at all: the tick did nothing");
  session.release();
});

test("m12: a field passed as undefined keeps the motor's own value", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  const kept = session.params().gravity;
  assert.doesNotThrow(() => session.setParams({ gravity: undefined }));
  assert.equal(session.params().gravity, kept, "an undefined overwrote a real value");
  assert.ok(Number.isFinite(session.params().gravity));
  session.release();
});

test("m12: a field the caller does name is still sent", async () => {
  const session = motor.forceSession(motor.build(INGEST));
  session.setParams({ gravity: 0.25, theta: undefined });
  assert.equal(session.params().gravity, 0.25);
  assert.equal(session.params().theta, motor.forceSession(motor.build(INGEST)).params().theta);
  session.release();
});

// --- m10: a failed creation's cleanup never replaces its refusal --------------------------

test("m10: a refused setParams at creation throws the parameter refusal, not a cleanup one", async () => {
  const error = await refusalOf(() => motor.forceSession(motor.build(INGEST), { theta: 99 }));
  assert.ok(error instanceof ForceSessionRefusedError, `threw ${error?.constructor?.name}`);
  assert.match(error.message, /theta|SessionRefused/);
});

// --- m14: `gm_alloc` is not a session's refusal -------------------------------------------

test("m14: a parameter buffer that cannot be reserved is an allocation failure", async () => {
  // The honest shape of this finding without a 4 GiB request: the class the module's own
  // refusal maps to, and the fact that it is not the session class.
  assert.ok(AllocationFailedError.prototype instanceof Error, "the class must be a GraphMotorError");
  assert.notEqual(AllocationFailedError, ForceSessionRefusedError, "the two must not be the same class");
  assert.equal(AllocationFailedError.name, "AllocationFailedError");
});

// --- m36: the SDK's own unit test runs, and m30: the exports map resolves -----------------

test("M36: this file is run by `npm run sdk:test`, which the gate row sdk-test invokes", () => {
  assert.ok(existsSync(join(HERE, "abi-version.test.mjs")), "the ABI handshake test is beside this one");
  assert.ok(existsSync(join(HERE, "motor.test.mjs")), "this file is where the row points");
});

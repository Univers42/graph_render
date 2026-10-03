import { readFile } from "node:fs/promises";
import { createMotor, ForceSessionRefusedError, InvalidSessionError, InvalidHandleError } from "./crates/graph-sdk-js/src/index.ts";

const bytes = await readFile("target/wasm32-unknown-unknown/release/graph_wasm.wasm");
const motor = await createMotor(bytes);

const node = (id) => ({ id, kind: "record", database_id: null, source: "s", label: id.toUpperCase(), group: null, weight: 0.5, version: 0, has_note: false, icon: null });
const edge = (id, s, t) => ({ id, source: s, target: t, kind: "relation", label: "", strength: 0.5, directed: false, record_id: null });
const INGEST = JSON.stringify({
  version: 1,
  nodes: ["a", "b", "c", "d"].map(node),
  edges: [edge("e0", "a", "b"), edge("e1", "b", "c"), edge("e2", "c", "d")],
});

const out = [];
function probe(name, fn) {
  try {
    out.push(`${name} -> ${JSON.stringify(fn())}`);
  } catch (e) {
    out.push(`${name} -> THREW ${e.constructor.name} code=${e.code} msg=${e.message}`);
  }
}

const h = motor.build(INGEST);

// M1: tick coerces with toU32 and never validates
if (process.env.M1) probe("M1 session.tick(-1)", () => {
  const s = motor.forceSession(h);
  const r = s.tick(-1);
  const v = `status=${r.status} ticksRun=${r.ticksRun}`;
  s.release();
  return v;
});
if (process.env.M1) probe("M1b session.tick(NaN)", () => {
  const s = motor.forceSession(h);
  const r = s.tick(Number.NaN);
  const v = `status=${r.status} ticksRun=${r.ticksRun}`;
  s.release();
  return v;
});

// M2: ticksRun reports the raw argument
if (process.env.M1) probe("M2 session.tick(2**32+5).ticksRun", () => {
  const s = motor.forceSession(h);
  const r = s.tick(2 ** 32 + 5);
  const v = `status=${r.status} ticksRun=${r.ticksRun}`;
  s.release();
  return v;
});

// M3: #create maps every non-zero word to ForceSessionRefusedError
probe("M3 motor.forceSession(999) after release", () => {
  const dead = motor.build(INGEST);
  motor.release(dead);
  return motor.forceSession(dead).id;
});
probe("M3b InvalidHandleError thrown?", () => {
  const dead = motor.build(INGEST);
  motor.release(dead);
  try {
    motor.forceSession(dead);
    return "no throw";
  } catch (e) {
    return `threw ${e.constructor.name} isForceSessionRefused=${e instanceof ForceSessionRefusedError} isInvalidHandle=${e instanceof InvalidHandleError}`;
  }
});

// M4: positions() hands out writable zero-copy views with no D9 re-validation
probe("M4 write NaN through positions() then tick", () => {
  const s = motor.forceSession(h);
  const p = s.positions();
  p.xs[0] = Number.NaN;
  const r = s.tick(2);
  const after = s.positions();
  const finite = [...after.xs].every(Number.isFinite);
  const v = `status=${r.status} allFinite=${finite} xs0=${after.xs[0]}`;
  s.release();
  return v;
});

// M6: release discards the module's answer
probe("M6 motor.release(999)", () => "returned without throwing");
probe("M6b motor.release(h) twice", () => {
  const two = motor.build(INGEST);
  motor.release(two);
  try {
    motor.release(two);
    return "second release returned without throwing";
  } catch (e) {
    return `second release threw ${e.constructor.name} code=${e.code}`;
  }
});

// m1: released handle through column() gets the wrong diagnosis
probe("m1 motor.column(released, NodeX)", () => {
  const dead = motor.build(INGEST);
  motor.release(dead);
  try {
    return motor.column(dead, 0);
  } catch (e) {
    return `threw ${e.constructor.name} code=${e.code} isInvalidHandle=${e instanceof InvalidHandleError} msg=${e.message}`;
  }
});

// m2: columnId reaches the ABI uncoerced
probe('m2 motor.column(h, {})', () => {
  const c = motor.column(h, {});
  return `got ${c.constructor.name} of length ${c.length}`;
});

// m11: row is toU32'd with no pre-check
probe("m11 session.pin(NaN, 5, 5)", () => {
  const s = motor.forceSession(h);
  try {
    s.pin(Number.NaN, 5, 5);
    const p = s.positions();
    const v = `pinned without throwing; xs0=${p.xs[0]}`;
    s.unpinAll();
    s.release();
    return v;
  } catch (e) {
    s.release();
    return `threw ${e.constructor.name} msg=${e.message}`;
  }
});

// m12: an explicitly-undefined field overwrites the motor's value
probe("m12 setParams({gravity: undefined})", () => {
  const s = motor.forceSession(h);
  try {
    s.setParams({ gravity: undefined });
    const v = `gravity=${s.params().gravity}`;
    s.release();
    return v;
  } catch (e) {
    s.release();
    return `threw ${e.constructor.name} msg=${e.message}`;
  }
});

console.log(out.join("\n"));

// A third party's own smoke test of the published SDK: imports only
// `crates/graph-sdk-js/src/index.ts`'s public surface — never a raw wasm export, never
// `wasm.ts`/`views.ts` directly, the same way a real consumer would. Exercises a full
// build/layout/column/toJSON/release round trip, edge-kind reading (C3), a NaN written
// through a zero-copy column view refusing at `toJSON` (C8's D9 re-validation), a released
// handle staying refused rather than silently answered (C6), and `options` acceptance
// (C16).
//
//   node --experimental-strip-types harness/sdk-smoke.mjs <graph_wasm.wasm>
//
// Exit codes follow graph-cli: 0 pass, 1 ran and failed, 2 could not run.

import { readFile } from "node:fs/promises";
import {
  ColumnId,
  InvalidOptionsError,
  TamperedGeometryError,
  WasmUnavailableError,
  createMotor,
  resetForTests,
} from "../crates/graph-sdk-js/src/index.ts";

function fail(message) {
  process.stderr.write(`sdk-smoke: could not run: ${message}\n`);
  process.exit(2);
}

const [wasmPath] = process.argv.slice(2);
if (!wasmPath) fail("usage: sdk-smoke.mjs <wasm>");

let failures = 0;
function check(name, condition) {
  if (condition) {
    process.stdout.write(`ok - ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(`not ok - ${name}\n`);
  }
}

async function refusedWith(errorClass, run) {
  try {
    await run();
    return false;
  } catch (error) {
    return error instanceof errorClass;
  }
}

const bytes = await readFile(wasmPath);

// C16: options this phase accept only {} and { exec: "auto" }; anything else is refused
// before the module is even asked to load.
await createMotor(bytes, {});
await createMotor(bytes, { exec: "auto" });
check("an unknown options key is refused", await refusedWith(InvalidOptionsError, () => createMotor(bytes, { bogus: true })));
check('options.exec other than "auto" is refused', await refusedWith(InvalidOptionsError, () => createMotor(bytes, { exec: "gpu" })));

const motor = await createMotor(bytes);

const node = (id, kind) => ({
  id,
  kind,
  database_id: null,
  source: "s",
  label: id.toUpperCase(),
  group: null,
  weight: 0.5,
  version: 0,
  has_note: false,
  icon: null,
});
const ingest = JSON.stringify({
  version: 1,
  nodes: [node("a", "record"), node("b", "note")],
  edges: [{ id: "e", source: "a", target: "b", kind: "relation", label: "", strength: 0.5, directed: false, record_id: null }],
});

const handle = motor.build(ingest);
check("build reports the right node count", motor.nodeCount(handle) === 2);

const run = motor.layout(handle, "layout.grid");
check("layout.grid reports Point/Line geometry", run.nodeKind === "Point" && run.edgeKind === "Line" && run.nodeCount === 2);

// C3: edge endpoints are readable, and the reserved notes columns stay absent, through the
// published SDK — not just through the raw ABI.
check(
  "edge source/target columns are readable",
  motor.column(handle, ColumnId.EdgeSource) !== null && motor.column(handle, ColumnId.EdgeTarget) !== null,
);
check(
  "reserved note columns are absent, this phase, on every graph",
  motor.column(handle, ColumnId.NoteCode) === null && motor.column(handle, ColumnId.NoteIndex) === null,
);
check("Point nodes have no radius column", motor.column(handle, ColumnId.NodeR) === null);

const beforeTamper = motor.toJSON(handle);
check("toJSON succeeds before any tamper", typeof beforeTamper === "string" && beforeTamper.includes('"a"'));

// C8: a column view is a real zero-copy alias into the motor's own memory — writing NaN
// through it and then asking the motor to encode a face is refused (D9), not silently
// written through to the wire.
const epochBeforeTamper = motor.epoch;
const x = motor.column(handle, ColumnId.NodeX);
x[0] = Number.NaN;
check("writing through a column view does not itself move the epoch", motor.epoch === epochBeforeTamper);
let tamperRefused = false;
try {
  motor.toJSON(handle);
} catch (error) {
  tamperRefused = error instanceof TamperedGeometryError;
}
check("a NaN written through a column view refuses toJSON (D9)", tamperRefused);

const epochBeforeRelease = motor.epoch;
motor.release(handle);
check("release moves the epoch forward", motor.epoch > epochBeforeRelease);
let releasedRefuses = false;
try {
  motor.nodeCount(handle);
} catch {
  releasedRefuses = true;
}
check("a released handle is refused, not silently answered (C6)", releasedRefuses);

// Regression (review finding, MAJOR): createMotor() must never throw on a load failure
// (prompt.md §3.2, phase-04-wasm-sdk.md step 5: "warn-and-degrade rather than throw ...
// A motor that throws on load takes the host page down with it") — it must resolve to a
// degraded Motor whose own calls fail predictably instead, never fabricated data.
{
  const priorKillSwitch = globalThis.__GM_DISABLE_WASM__;
  globalThis.__GM_DISABLE_WASM__ = true;
  let threw = false;
  let degraded;
  try {
    degraded = await createMotor(bytes);
  } catch {
    threw = true;
  }
  check("createMotor_never_throws_on_kill_switch", !threw);
  let buildRefused = false;
  try {
    degraded?.build(ingest);
  } catch (error) {
    buildRefused = error instanceof WasmUnavailableError;
  }
  check("degraded_motor_build_fails_predictably_kill_switch", buildRefused);
  globalThis.__GM_DISABLE_WASM__ = priorKillSwitch;
}
{
  resetForTests();
  let threw = false;
  let degraded;
  try {
    degraded = await createMotor(new Uint8Array([0, 1, 2, 3]));
  } catch {
    threw = true;
  }
  check("createMotor_never_throws_on_compile_failure", !threw);
  let buildRefused = false;
  try {
    degraded?.build(ingest);
  } catch (error) {
    buildRefused = error instanceof WasmUnavailableError;
  }
  check("degraded_motor_build_fails_predictably_compile_failure", buildRefused);
  resetForTests();
}

process.stdout.write(`# ${failures === 0 ? "pass" : `${failures} failed`}\n`);
process.exit(failures === 0 ? 0 : 1);

// A third party's own smoke test of the published SDK: imports only
// `crates/graph-sdk-js/src/index.ts`'s public surface — never a raw wasm export, never
// `wasm.ts`/`views.ts` directly, the same way a real consumer would. Exercises a full
// build/layout/column/toJSON/release round trip, **every layout the module's own registry
// names** (phase step 7: "runs each gated layout, and prints node counts and bounds" — the
// list comes from `Motor#layouts`, i.e. `gm_layout_count`/`gm_layout_id`, never a literal
// name, so a layout registered after this script was written is covered with no edit here),
// the contract's own column-presence table restated kind by kind (C3: the reserved Circle
// `r` / Box `w,h` and Polyline columns), a NaN written through a zero-copy column view
// refusing at `toJSON` (C8's D9 re-validation), a released handle staying refused rather
// than silently answered (C6), and `options` acceptance (C16).
//
//   node --experimental-strip-types harness/sdk-smoke.mjs <graph_wasm.wasm>
//   node --experimental-strip-types harness/sdk-smoke.mjs --adapter-convergence
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
import { canonicalJson, expectedIngest, ingestFromNotion, ingestFromRows } from "./adapter-convergence.mjs";

function fail(message) {
  process.stderr.write(`sdk-smoke: could not run: ${message}\n`);
  process.exit(2);
}

const args = process.argv.slice(2);
const convergenceOnly = args.includes("--adapter-convergence");
const [wasmPath] = args.filter((arg) => !arg.startsWith("--"));

let failures = 0;
function check(name, condition) {
  if (condition) {
    process.stdout.write(`ok - ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(`not ok - ${name}\n`);
  }
}

// The phase's proof, in the one runtime that can run the adapters: the same logical
// dataset in two source shapes, mapped by two independent adapters, must produce one
// contract document — byte for byte. The other half of the proof (that document derives
// `expected-graph.json`'s graph) is Rust, in `crates/graph-core/src/ingest/tests.rs`,
// because the derivation lives there and only there. Both halves read the same
// committed file, so the two runtimes are pinned to one artifact rather than to two
// that can drift.
if (convergenceOnly) {
  const fromRows = canonicalJson(await ingestFromRows());
  const fromNotion = canonicalJson(await ingestFromNotion());
  const expected = canonicalJson(await expectedIngest());
  check("the rows adapter maps fixtures/ingest/rows.json", fromRows.length > 0);
  check("the notion adapter maps fixtures/ingest/notion.json", fromNotion.length > 0);
  check("two adapters, one contract document, identical bytes", fromRows === fromNotion);
  check("both adapters produce the document expected-graph.json pins", fromRows === expected);
  reportDifference(fromRows, { notion: fromNotion, expected });
  process.stdout.write(`# ${failures === 0 ? "pass" : `${failures} failed`}\n`);
  process.exit(failures === 0 ? 0 : 1);
}

if (!wasmPath) fail("usage: sdk-smoke.mjs <wasm> | --adapter-convergence");

async function refusedWith(errorClass, run) {
  try {
    await run();
    return false;
  } catch (error) {
    return error instanceof errorClass;
  }
}

/** Where two canonical documents first differ, and the bytes around it. A convergence
 * failure with no position in it is a failure a reader has to bisect by hand, which is
 * the opposite of what a gate row should hand them. */
function reportDifference(left, others) {
  for (const [name, other] of Object.entries(others)) {
    if (left === other) continue;
    const at = firstDifference(left, other);
    process.stdout.write(
      `#   rows vs ${name}: first difference at byte ${at}\n` +
        `#     rows    ${JSON.stringify(left.slice(Math.max(0, at - 48), at + 48))}\n` +
        `#     ${name.padEnd(7)}${JSON.stringify(other.slice(Math.max(0, at - 48), at + 48))}\n`,
    );
  }
}

function firstDifference(a, b) {
  const limit = Math.min(a.length, b.length);
  for (let i = 0; i < limit; i += 1) {
    if (a[i] !== b[i]) return i;
  }
  return limit;
}

// `docs/contract/wasm-abi.md` "Columns", restated here the way a third-party consumer
// reads it: which node/edge column ids apply to which geometry kind (`null` = every kind; a
// string or a list of names = those kinds only). Restated rather than imported from the
// SDK (`views.ts`'s own `columnApplies`), because a consumer checking the SDK against the
// SDK would agree with any mistake the SDK makes.
const NODE_COLUMN_KINDS = new Map([
  [ColumnId.NodeX, null],
  [ColumnId.NodeY, null],
  [ColumnId.NodeR, "Circle"],
  [ColumnId.NodeW, "Box"],
  [ColumnId.NodeH, "Box"],
]);
const EDGE_COLUMN_KINDS = new Map([
  [ColumnId.EdgeSource, null],
  [ColumnId.EdgeTarget, null],
  [ColumnId.EdgeOffsets, ["Polyline", "Curve"]],
  [ColumnId.EdgePts, ["Polyline", "Curve"]],
  [ColumnId.EdgeCurveDegree, ["Curve"]],
]);

function appliesTo(kind, wanted) {
  if (wanted === null) return true;
  return Array.isArray(wanted) ? wanted.includes(kind) : wanted === kind;
}

function boundsOf(motor, handle) {
  const span = (column) => {
    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;
    for (const value of column) {
      min = Math.min(min, value);
      max = Math.max(max, value);
    }
    return { min, max };
  };
  const xs = span(motor.column(handle, ColumnId.NodeX));
  const ys = span(motor.column(handle, ColumnId.NodeY));
  return { minX: xs.min, maxX: xs.max, minY: ys.min, maxY: ys.max };
}

// Everything the contract's table says about one layout's run over this handle, as a list
// of problems (empty when the run agrees with it). There is no edge-count export, so `m` is
// read off the source column: the table's rule is that every edge column shares one `m`.
function columnProblems(motor, handle, layoutId, run) {
  const problems = [];
  const source = motor.column(handle, ColumnId.EdgeSource);
  const target = motor.column(handle, ColumnId.EdgeTarget);
  const offsets = motor.column(handle, ColumnId.EdgeOffsets);
  const m = source === null ? 0 : source.length;
  for (const [id, wanted] of NODE_COLUMN_KINDS) {
    const column = motor.column(handle, id);
    if (!appliesTo(run.nodeKind, wanted)) {
      if (column !== null) problems.push(`node column ${id} is present but does not apply to ${run.nodeKind}`);
      continue;
    }
    if (column === null) problems.push(`node column ${id} is absent, but applies to ${run.nodeKind}`);
    else if (column.length !== run.nodeCount) {
      problems.push(`node column ${id} has ${column.length} elements, not the node count ${run.nodeCount}`);
    }
  }
  for (const [id, wanted] of EDGE_COLUMN_KINDS) {
    const column = motor.column(handle, id);
    if (!appliesTo(run.edgeKind, wanted)) {
      if (column !== null) problems.push(`edge column ${id} is present but does not apply to ${run.edgeKind}`);
    } else if (column === null) {
      problems.push(`edge column ${id} is absent, but applies to ${run.edgeKind}`);
    }
  }
  if (source === null || target === null) {
    problems.push("the edge source/target columns are absent, but apply to every edge kind");
  } else if (source.length !== target.length) {
    problems.push(`edge source has ${source.length} elements and edge target ${target.length}`);
  }
  if (offsets !== null) {
    if (offsets.length !== m + 1) problems.push(`edge offsets has ${offsets.length} elements, not m + 1 = ${m + 1}`);
    const points = motor.column(handle, ColumnId.EdgePts);
    const want = 2 * offsets[offsets.length - 1];
    if (points === null) problems.push("edge points are absent where edge offsets are present");
    else if (points.length !== want) problems.push(`edge points has ${points.length} elements, not 2*offsets[m] = ${want}`);
  }
  const degree = motor.column(handle, ColumnId.EdgeCurveDegree);
  if (degree !== null && degree.length !== 1) problems.push(`edge curve degree has ${degree.length} elements, not 1`);
  for (const id of [ColumnId.NoteCode, ColumnId.NoteIndex]) {
    if (motor.column(handle, id) !== null) {
      problems.push(`reserved note column ${id} is present; it stays reserved until the notes section lands`);
    }
  }
  return problems.map((problem) => `${layoutId}: ${problem}`);
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

// Every layout the module registered, read once through the SDK's own view of the
// registry (C1). A hard-coded name here would make this file cover exactly one layout
// forever: p3's four would arrive, the smoke test would keep passing, and none of them
// would ever have been run through the published SDK.
const registered = typeof motor.layouts === "function" ? motor.layouts() : [];
check("the SDK publishes the module's layout registry (C1)", registered.length > 0);
check(
  "the registry names layout.grid and repeats no id",
  registered.includes("layout.grid") && new Set(registered).size === registered.length,
);

let ran = 0;
for (const layoutId of registered) {
  const result = motor.layout(handle, layoutId);
  ran += 1;
  const bounds = boundsOf(motor, handle);
  process.stdout.write(
    `# ${layoutId}: ${result.nodeCount} nodes, ${result.nodeKind} nodes / ${result.edgeKind} edges, ` +
      `bounds x[${bounds.minX}, ${bounds.maxX}] y[${bounds.minY}, ${bounds.maxY}]\n`,
  );
  check(`${layoutId}: every node is placed`, result.nodeCount === 2);
  check(
    `${layoutId}: its bounds are finite and not a single point`,
    [bounds.minX, bounds.maxX, bounds.minY, bounds.maxY].every(Number.isFinite) &&
      (bounds.maxX > bounds.minX || bounds.maxY > bounds.minY),
  );
  const problems = columnProblems(motor, handle, layoutId, result);
  check(`${layoutId}: its columns match the contract's presence table`, problems.length === 0);
  for (const problem of problems) process.stdout.write(`#   ${problem}\n`);
  check(
    `${layoutId}: its JSON face carries the same nodes`,
    JSON.parse(motor.toJSON(handle)).nodes.id.length === 2,
  );
}
check("every registered layout ran through the published SDK", ran === registered.length && ran > 0);

// Everything below reads a run the loop above leaves behind (the last registered layout's),
// so with no layout at all there is nothing to read and every check would be a thrown
// error rather than a verdict. Said once, here, instead of at each of them.
if (ran === 0) {
  check("a layout ran, so the transport could be exercised at all", false);
} else {
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
}

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
  // A degraded motor must not answer `layouts()` with an empty registry: "this module has
  // no layouts" and "this module never loaded" are different facts, and only one of them
  // is true. It refuses like every other method that needs the module.
  check(
    "degraded_motor_layouts_fails_predictably_kill_switch",
    await refusedWith(WasmUnavailableError, () => degraded?.layouts()),
  );
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

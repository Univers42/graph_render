// Shared checks and helpers for the SDK smoke test (`harness/sdk-smoke.mjs`).

import {
  ColumnId,
} from "../../crates/graph-sdk-js/src/index.ts";

// `ColumnId` is imported for exactly one thing: `columnIdProblems` below compares the SDK's
// own constants against this file's literals. Nothing here calls into `views.ts`.

export function fail(message) {
  process.stderr.write(`sdk-smoke: could not run: ${message}\n`);
  process.exit(2);
}

let failures = 0;
export function check(name, condition, detail = "") {
  if (condition) {
    process.stdout.write(`ok - ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(`not ok - ${name}${detail ? `\n#   ${detail}` : ""}\n`);
  }
}

export async function refusedWith(errorClass, run) {
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
export function reportDifference(left, others) {
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
// string or a list of names = those kinds only; `{dim: n}` = every kind, but only on a run
// whose `dim` says 3D or 2D). Restated rather than imported from the SDK (`views.ts`'s own
// `columnApplies`), because a consumer checking the SDK against the SDK would agree with
// any mistake the SDK makes.
//
// The ids are the contract's **literals**, not `ColumnId.*`: C3's "append-only, never
// renumbered" is a claim about the numbers, and a table keyed by the SDK's own constants
// restates applicability while agreeing with any renumbering the SDK made (m90).
// `columnIdProblems` below is what pins the binding.
const COLUMN_IDS = new Map([
  ["NodeX", 0],
  ["NodeY", 1],
  ["NodeR", 2],
  ["NodeW", 3],
  ["NodeH", 4],
  ["EdgeSource", 5],
  ["EdgeTarget", 6],
  ["NoteCode", 7],
  ["NoteIndex", 8],
  ["EdgeOffsets", 9],
  ["EdgePts", 10],
  ["EdgeCurveDegree", 11],
  ["NodeZ", 12],
]);
const ID = Object.fromEntries(COLUMN_IDS);

/** Why the SDK's `ColumnId` does not spell the contract's id for a name, or `null`. */
export function columnIdProblems(sdkColumnId) {
  const problems = [];
  for (const [name, id] of COLUMN_IDS) {
    if (sdkColumnId[name] !== id) {
      problems.push(`ColumnId.${name} is ${String(sdkColumnId[name])}, not the contract's ${id}`);
    }
  }
  for (const name of Object.keys(sdkColumnId)) {
    if (!COLUMN_IDS.has(name)) problems.push(`ColumnId.${name} is a name the contract does not name`);
  }
  return problems;
}

const NODE_COLUMN_KINDS = new Map([
  [ID.NodeX, null],
  [ID.NodeY, null],
  [ID.NodeR, "Circle"],
  [ID.NodeW, "Box"],
  [ID.NodeH, "Box"],
  // The one row that keys on the run's dimension rather than its node kind: every node
  // kind carries a z on a 3D run, and a 2D run has none at all — absent, not a
  // zero-length column a caller might read as a plane at depth 0 (M30).
  [ID.NodeZ, { dim: 1 }],
]);
const EDGE_COLUMN_KINDS = new Map([
  [ID.EdgeSource, null],
  [ID.EdgeTarget, null],
  [ID.EdgeOffsets, ["Polyline", "Curve"]],
  [ID.EdgePts, ["Polyline", "Curve"]],
  [ID.EdgeCurveDegree, ["Curve"]],
]);

/** Whether `wanted` (a table entry) covers a run of node kind `kind` and dimension `dim`.
 *  An array is an object, so the list form is tested first: `["Polyline","Curve"]` is a
 *  list of kinds, not a `{dim}` rule with a `dim` of `"Polyline"`. */
function appliesTo(kind, dim, wanted) {
  if (Array.isArray(wanted)) return wanted.includes(kind);
  if (wanted === null) return true;
  if (typeof wanted === "object") return wanted.dim === dim;
  return wanted === kind;
}

export function boundsOf(motor, handle) {
  const span = (column) => {
    let min = Number.POSITIVE_INFINITY;
    let max = Number.NEGATIVE_INFINITY;
    for (const value of column) {
      min = Math.min(min, value);
      max = Math.max(max, value);
    }
    return { min, max };
  };
  const xs = span(motor.column(handle, ID.NodeX));
  const ys = span(motor.column(handle, ID.NodeY));
  // A 2D run reads NodeZ as null; a 3D one (layout.hierarchical3d stacks its levels on z) is
  // a single point only if z is flat too.
  const z = motor.column(handle, ID.NodeZ);
  const zs = z === null ? { min: 0, max: 0 } : span(z);
  return { minX: xs.min, maxX: xs.max, minY: ys.min, maxY: ys.max, minZ: zs.min, maxZ: zs.max };
}

// Everything the contract's table says about one layout's run over this handle, as a list
// of problems (empty when the run agrees with it). There is no edge-count export, so `m` is
// read off the source column: the table's rule is that every edge column shares one `m`.
export function columnProblems(motor, handle, layoutId, run) {
  const problems = [];
  const source = motor.column(handle, ID.EdgeSource);
  const target = motor.column(handle, ID.EdgeTarget);
  const offsets = motor.column(handle, ID.EdgeOffsets);
  const m = source === null ? 0 : source.length;
  for (const [id, wanted] of NODE_COLUMN_KINDS) {
    const column = motor.column(handle, id);
    if (!appliesTo(run.nodeKind, run.dim, wanted)) {
      if (column !== null) problems.push(`node column ${id} is present but does not apply to ${run.nodeKind} at dim ${run.dim}`);
      continue;
    }
    if (column === null) problems.push(`node column ${id} is absent, but applies to ${run.nodeKind} at dim ${run.dim}`);
    else if (column.length !== run.nodeCount) {
      problems.push(`node column ${id} has ${column.length} elements, not the node count ${run.nodeCount}`);
    }
  }
  for (const [id, wanted] of EDGE_COLUMN_KINDS) {
    const column = motor.column(handle, id);
    if (!appliesTo(run.edgeKind, run.dim, wanted)) {
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
    const points = motor.column(handle, ID.EdgePts);
    const want = 2 * offsets[offsets.length - 1];
    if (points === null) problems.push("edge points are absent where edge offsets are present");
    else if (points.length !== want) problems.push(`edge points has ${points.length} elements, not 2*offsets[m] = ${want}`);
  }
  const degree = motor.column(handle, ID.EdgeCurveDegree);
  if (degree !== null && degree.length !== 1) problems.push(`edge curve degree has ${degree.length} elements, not 1`);
  for (const id of [ID.NoteCode, ID.NoteIndex]) {
    if (motor.column(handle, id) !== null) {
      problems.push(`reserved note column ${id} is present; it stays reserved until the notes section lands`);
    }
  }
  return problems.map((problem) => `${layoutId}: ${problem}`);
}

export function finish() {
  process.stdout.write(`# ${failures === 0 ? "pass" : `${failures} failed`}\n`);
  // `process.exitCode`, not `process.exit`: POSIX stdout is asynchronous on a pipe, so
  // `write` followed by `exit` truncates — a `not ok` line past the pipe buffer was lost
  // from the gate log while the exit code still said 1 (m89). Setting the code lets node
  // drain stdout before it leaves.
  process.exitCode = failures === 0 ? 0 : 1;
}

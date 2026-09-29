// Shared checks and helpers for the SDK smoke test (`harness/sdk-smoke.mjs`).

import {
  ColumnId,
} from "../../crates/graph-sdk-js/src/index.ts";

export function fail(message) {
  process.stderr.write(`sdk-smoke: could not run: ${message}\n`);
  process.exit(2);
}

let failures = 0;
export function check(name, condition) {
  if (condition) {
    process.stdout.write(`ok - ${name}\n`);
  } else {
    failures += 1;
    process.stdout.write(`not ok - ${name}\n`);
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
  const xs = span(motor.column(handle, ColumnId.NodeX));
  const ys = span(motor.column(handle, ColumnId.NodeY));
  return { minX: xs.min, maxX: xs.max, minY: ys.min, maxY: ys.max };
}

// Everything the contract's table says about one layout's run over this handle, as a list
// of problems (empty when the run agrees with it). There is no edge-count export, so `m` is
// read off the source column: the table's rule is that every edge column shares one `m`.
export function columnProblems(motor, handle, layoutId, run) {
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


export function finish() {
  process.stdout.write(`# ${failures === 0 ? "pass" : `${failures} failed`}\n`);
  process.exit(failures === 0 ? 0 : 1);
}

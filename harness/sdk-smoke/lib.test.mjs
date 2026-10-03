// The restated column table, pinned against a stub motor: the negative cases the live
// smoke could not fail on its own, because every registered layout happened to agree with
// the SDK. These drive `columnProblems` with a motor that does not.
//
// M30: the table had no `NodeZ` row at all, so a `views.ts` that answered `true` for
// NodeZ on every run — a 2D run exposed with a z column — passed the whole loop.
// m90: the table was keyed by the SDK's own `ColumnId.*`, so it restated applicability
// while agreeing with any renumbering; `columnIdProblems` is what pins the numbers.

import { test } from "node:test";
import assert from "node:assert/strict";

import { ColumnId } from "../../crates/graph-sdk-js/src/index.ts";
import { columnIdProblems, columnProblems } from "./lib.mjs";

const NODE_Z = 12;

/** A motor whose columns are exactly `present`: id → element count (a zero-filled column)
 *  or an array (its own values, for the CSR rows whose last offset is load-bearing), or
 *  `null` for absent. */
function motorWith(present) {
  const columns = new Map(Object.entries(present).map(([id, shape]) => [Number(id), shape]));
  return {
    column: (_handle, id) => {
      const shape = columns.get(id);
      if (shape === undefined || shape === null) return null;
      return Array.isArray(shape) ? shape : new Float64Array(shape);
    },
  };
}

const POINT_RUN = { nodeKind: "Point", edgeKind: "Line", nodeCount: 2, dim: 0 };
const POINT_3D_RUN = { nodeKind: "Point", edgeKind: "Line", nodeCount: 2, dim: 1 };

test("m30: a z column on a 2D run is a reported problem, not a pass", () => {
  // Under the old table this layout produced no problem at all: there was no NodeZ row.
  const problems = columnProblems(
    motorWith({ [ColumnId.NodeX]: 2, [ColumnId.NodeY]: 2, [NODE_Z]: 2, [ColumnId.EdgeSource]: 1, [ColumnId.EdgeTarget]: 1 }),
    1,
    "layout.grid",
    POINT_RUN,
  );
  assert.deepEqual(problems, ["layout.grid: node column 12 is present but does not apply to Point at dim 0"]);
});

test("m30: a 3D run with no z column is a reported problem, not a pass", () => {
  const problems = columnProblems(
    motorWith({ [ColumnId.NodeX]: 2, [ColumnId.NodeY]: 2, [ColumnId.EdgeSource]: 1, [ColumnId.EdgeTarget]: 1 }),
    1,
    "layout.hierarchical3d",
    POINT_3D_RUN,
  );
  assert.deepEqual(problems, ["layout.hierarchical3d: node column 12 is absent, but applies to Point at dim 1"]);
});

test("m30: a 3D run with a z column is not a problem", () => {
  const problems = columnProblems(
    motorWith({ [ColumnId.NodeX]: 2, [ColumnId.NodeY]: 2, [NODE_Z]: 2, [ColumnId.EdgeSource]: 1, [ColumnId.EdgeTarget]: 1 }),
    1,
    "layout.hierarchical3d",
    POINT_3D_RUN,
  );
  assert.deepEqual(problems, []);
});

test("m30: the kind-keyed rows still key on kind, not on dim", () => {
  // The `Array.isArray` ordering matters: a list of kinds is an object too, and reading it
  // as a `{dim}` rule reported every Polyline's offsets column as inapplicable.
  const problems = columnProblems(
    motorWith({
      [ColumnId.NodeX]: 2,
      [ColumnId.NodeY]: 2,
      [ColumnId.EdgeSource]: 1,
      [ColumnId.EdgeTarget]: 1,
      [ColumnId.EdgeOffsets]: [0, 2],
      [ColumnId.EdgePts]: 4,
    }),
    1,
    "layout.tree.tidy",
    { nodeKind: "Point", edgeKind: "Polyline", nodeCount: 2, dim: 0 },
  );
  assert.deepEqual(problems, []);
});

test("m90: the SDK's ColumnId must spell the contract's numbers", () => {
  assert.deepEqual(columnIdProblems(ColumnId), []);
  // The review's failing input: `NodeR: 3, NodeW: 2` swapped, everything else kept.
  const swapped = { ...ColumnId, NodeR: 3, NodeW: 2 };
  const problems = columnIdProblems(swapped);
  assert.equal(problems.length, 2);
  assert.match(problems[0], /ColumnId\.NodeR is 3, not the contract's 2|ColumnId\.NodeR/);
  assert.match(problems[1], /ColumnId\.NodeW/);
});

test("m90: a name the contract does not name is reported", () => {
  const problems = columnIdProblems({ ...ColumnId, NodeW2: 99 });
  assert.deepEqual(problems, ["ColumnId.NodeW2 is a name the contract does not name"]);
});
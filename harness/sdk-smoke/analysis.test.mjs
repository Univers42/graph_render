// The analysis element-type table, pinned on its own: the review's failing input is a
// producer that declares `analysis.centrality.closeness` a `u32`, and under the old
// two-value check (`kind === "f64" || kind === "u32"`) that passed every loop assertion —
// the `u32` branch only adds an integrality test the closeness values satisfy.
//
// The table is restated from `docs/contract/wasm-abi.md`'s analysis face, so it is a
// claim about the contract, not about this module's current output.

import { test } from "node:test";
import assert from "node:assert/strict";

import { elementTypeProblem } from "./analysis.mjs";

test("M34: every registered analysis has an element type the table names", () => {
  // The eight the SDK's registry publishes, each with the type its face declares.
  const faces = {
    "analysis.centrality.closeness": "f64",
    "analysis.centrality.degree": "f64",
    "analysis.centrality.eigenvector": "f64",
    "analysis.centrality.betweenness": "f64",
    "analysis.communities.louvain": "u32",
    "analysis.components.weak": "u32",
    "analysis.components.strong": "u32",
    "analysis.depth.bfs": "u32",
  };
  for (const [id, kind] of Object.entries(faces)) {
    assert.equal(elementTypeProblem(id, kind), null, id);
  }
});

test("M34: a producer that declares closeness a u32 is a reported problem", () => {
  // The failing input, verbatim from the review.
  assert.equal(
    elementTypeProblem("analysis.centrality.closeness", "u32"),
    "declared u32, contract says f64",
  );
  assert.equal(
    elementTypeProblem("analysis.components.weak", "f64"),
    "declared f64, contract says u32",
  );
});

test("M34: an id the table does not name is a problem, not a silent pass", () => {
  assert.equal(elementTypeProblem("analysis.components.none", "u32"), "analysis.components.none is not named in the element-type table");
});
// The tooltip line under the cursor, with an analysis applied. What matters here
// is that the number the node's COLOUR stands for is the number the tooltip
// prints: a ramp whose values cannot be read off the canvas is a picture without
// a scale, and a face left over from another graph must add nothing rather than
// another node's value.

import assert from "node:assert/strict";
import { test } from "node:test";

import { tooltipText } from "../src/ui/tooltip.ts";
import type { AnalysisResult } from "../../crates/graph-sdk-js/src/index.ts";
import type { NodeStyle } from "../src/render/palette.ts";

const STYLES: readonly NodeStyle[] = [
  { fill: "#e0937a", shape: "disc", kind: "record", label: "Node 0", group: "Alpha" },
  { fill: "#7fb2c4", shape: "note", kind: "note", label: "Node 1", group: null },
];

function face(over: Partial<AnalysisResult> & Pick<AnalysisResult, "kind" | "values">): AnalysisResult {
  return { id: "analysis.test", nodeCount: over.values.length, ...over };
}

test("with no analysis the line is the label, the kind and the dense index", () => {
  assert.equal(tooltipText(STYLES, 0, null), "Node 0 · record · #0");
});

test("with an analysis the line appends that node's own value", () => {
  assert.equal(
    tooltipText(STYLES, 1, face({ kind: "f64", values: [0.5, 0.25] })),
    "Node 1 · note · #1 · 0.250000",
  );
});

test("a labelling prints its integer, not a six-decimal magnitude", () => {
  assert.equal(
    tooltipText(STYLES, 0, face({ kind: "u32", values: [7, 0] })),
    "Node 0 · record · #0 · 7",
  );
});

test("a dense index with no style still prints, with its value", () => {
  assert.equal(
    tooltipText([], 3, face({ kind: "f64", values: [0, 0, 0, 0.125] })),
    "#3 · 0.125000",
  );
});

test("an index the face does not cover adds no value rather than another node's", () => {
  const short = face({ kind: "f64", values: [0.5] });
  assert.equal(tooltipText(STYLES, 1, short), "Node 1 · note · #1");
});

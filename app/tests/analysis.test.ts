// The overlay projection: an analysis face becomes node fills, a legend, and the
// panel's scalar readouts. The regressions these pin are the two ways a colouring
// can lie — a `f64` face binned as if its values were groups, and a `u32` face
// ramped as if its values were magnitudes — plus the domain edge cases (one
// value, an empty face, a negative face) where a min/max sweep would divide by
// zero and hand the canvas a NaN.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  analysisLegend,
  coversGraph,
  describeAnalysis,
  fillFor,
  fillsFor,
  formatScore,
  formatValue,
  groupPalette,
  hoverValue,
  normalise,
  rampFills,
  valueAt,
} from "../src/core/analysis.ts";
import { GROUP_COLOURS, RAMP_STOPS, rampColour } from "../src/render/analysisColours.ts";
import type { AnalysisResult } from "../../crates/graph-sdk-js/src/index.ts";

function face(over: Partial<AnalysisResult> & Pick<AnalysisResult, "kind" | "values">): AnalysisResult {
  return { id: "analysis.test", nodeCount: over.values.length, ...over };
}

test("a centrality is ramped across its own value domain, cold at the minimum", () => {
  const fills = rampFills(face({ kind: "f64", values: [0, 1, 2, 3] }));
  assert.deepEqual(fills, [RAMP_STOPS[0], rampColour(1 / 3), rampColour(2 / 3), RAMP_STOPS[4]]);
  assert.notEqual(fills[0], fills[3]);
});

test("the ramp spans the values it was given, not 0..1 of the unit interval", () => {
  // The same values shifted by 100 must not change the picture: the domain is
  // the run's own min..max, so a degree centrality of 3000..4000 reads the same
  // as one of 3..4.
  const low = rampFills(face({ kind: "f64", values: [3, 4] }));
  const high = rampFills(face({ kind: "f64", values: [3000, 4000] }));
  assert.deepEqual(high, low);
});

test("a domain of one value puts every node at the middle of the ramp, not at zero", () => {
  const fills = rampFills(face({ kind: "f64", values: [7, 7, 7] }));
  assert.equal(new Set(fills).size, 1);
  assert.equal(fills[0], rampColour(0.5));
});

test("a negative centrality keeps its sign in the fill rather than running off the ramp", () => {
  const fills = rampFills(face({ kind: "f64", values: [-2, 0, 2] }));
  assert.deepEqual(fills, [RAMP_STOPS[0], rampColour(0.5), RAMP_STOPS[4]]);
});

test("a labelling is coloured by GROUP, with each group one palette colour", () => {
  const fills = groupPalette(face({ kind: "u32", values: [0, 1, 0, 2, 1] }));
  assert.deepEqual([...new Set(fills)], [...new Set(fills)], "fills are strings");
  assert.equal(fills[0], fills[2], "two nodes in the same group share a colour");
  assert.equal(fills[1], fills[4]);
  assert.notEqual(fills[0], fills[1]);
  assert.notEqual(fills[1], fills[3]);
});

test("group colour follows the group RANK, so a relabelling from 5,9,17 reads the same", () => {
  // Louvain hands out arbitrary community numbers. The colour must come from the
  // rank of the group among the ASCENDING distinct groups, so the smallest group
  // is always the first palette entry — a pass that coloured by the number
  // itself would give group 17 the 9th colour and break the legend.
  const relabelled = groupPalette(face({ kind: "u32", values: [17, 5, 9] }));
  const dense = groupPalette(face({ kind: "u32", values: [0, 1, 2] }));
  assert.equal(relabelled[1], GROUP_COLOURS[0], "the smallest group, 5, takes the first colour");
  assert.equal(relabelled[2], GROUP_COLOURS[1]);
  assert.equal(relabelled[0], GROUP_COLOURS[2]);
  assert.deepEqual([...relabelled].sort(), [...dense].sort());
});

test("an empty face produces no fills and no crash", () => {
  assert.deepEqual(rampFills(face({ kind: "f64", values: [] })), []);
  assert.deepEqual(groupPalette(face({ kind: "u32", values: [] })), []);
});

test("fillsFor dispatches on the face's OWN kind, and a swap is visible", () => {
  // The one place the two projections meet, so a kind test inverted here would
  // ramp every community id and bin every centrality — each face is checked
  // against BOTH projections and must match only its own.
  const magnitude = face({ kind: "f64", values: [0, 1, 2] });
  const labelling = face({ kind: "u32", values: [0, 1, 2] });
  assert.deepEqual(fillsFor(magnitude), rampFills(magnitude));
  assert.notDeepEqual(fillsFor(magnitude), groupPalette(magnitude));
  assert.deepEqual(fillsFor(labelling), groupPalette(labelling));
  assert.notDeepEqual(fillsFor(labelling), rampFills(labelling));
});

test("normalise maps a value into 0..1 across the domain and pins a flat domain at 0.5", () => {
  assert.equal(normalise(0, 0, 10), 0);
  assert.equal(normalise(10, 0, 10), 1);
  assert.equal(normalise(2.5, 0, 10), 0.25);
  assert.equal(normalise(5, 5, 5), 0.5);
  assert.ok(Object.is(normalise(5, 5, 5), 0.5));
});

test("the legend for a ramped face names the two ends it spans, in the ramp's own colours", () => {
  assert.deepEqual(analysisLegend(face({ kind: "f64", values: [0.25, 0.75] })), [
    { label: "0.250000", fill: RAMP_STOPS[0] },
    { label: "0.750000", fill: RAMP_STOPS[4] },
  ]);
});

test("a flat f64 face gets a ONE-entry legend, in the one colour it paints", () => {
  const legend = analysisLegend(face({ kind: "f64", values: [3, 3] }));
  assert.deepEqual(legend, [{ label: "3.000000", fill: rampColour(0.5) }]);
  assert.equal(legend[0].fill, rampFills(face({ kind: "f64", values: [3, 3] }))[0]);
});

test("the legend for a grouping names every group with the colour that group has", () => {
  const result = face({ kind: "u32", values: [3, 3, 8] });
  const fills = groupPalette(result);
  assert.deepEqual(analysisLegend(result), [
    { label: "group 3", fill: fills[0] },
    { label: "group 8", fill: fills[2] },
  ]);
});

test("the legend collapses a long run of groups to the first, with the count", () => {
  const values = Array.from({ length: 40 }, (_, i) => i);
  const legend = analysisLegend(face({ kind: "u32", values }));
  assert.equal(legend.length, 9, "eight groups and the count");
  assert.equal(legend[0].label, "group 0");
  assert.equal(legend[7].label, "group 7");
  assert.equal(legend[8].label, "32 more groups");
  assert.equal(legend[8].fill, "transparent");
});

test("describeAnalysis reports the face's own scalars and never invents one", () => {
  const plain = describeAnalysis(face({ kind: "f64", values: [0, 1] }));
  assert.deepEqual(plain.map((row) => row.label), ["id", "kind", "nodes", "min", "max"]);
  assert.equal(plain.find((row) => row.label === "min")?.value, "0.000000");

  const flagged = describeAnalysis(
    face({ id: "analysis.centrality.eigenvector", kind: "f64", values: [0.5, 0.5], converged: false }),
  );
  assert.equal(flagged.find((row) => row.label === "converged")?.value, "false");

  const louvain = describeAnalysis(
    face({ id: "analysis.communities.louvain", kind: "u32", values: [0, 1], modularity: 0.3045267489711934 }),
  );
  assert.equal(louvain.find((row) => row.label === "modularity")?.value, "0.304527");

  const depth = describeAnalysis(face({ kind: "u32", values: [0, 2], max: 2 }));
  assert.equal(depth.find((row) => row.label === "max level")?.value, "2");
});

test("describeAnalysis on a u32 face names the group count rather than a min/max pair", () => {
  const rows = describeAnalysis(face({ kind: "u32", values: [0, 1, 1, 5] }));
  assert.deepEqual(rows.map((row) => row.label), ["id", "kind", "nodes", "groups"]);
  assert.equal(rows[3].value, "3");
});

test("describeAnalysis on an empty face says so instead of printing NaN", () => {
  const rows = describeAnalysis(face({ kind: "f64", values: [] }));
  assert.deepEqual(rows.map((row) => row.label), ["id", "kind", "nodes", "empty"]);
});

test("formatValue prints a labelling as an integer and a magnitude to six places", () => {
  assert.equal(formatValue(face({ kind: "u32", values: [0] }), 3), "3");
  assert.equal(formatValue(face({ kind: "f64", values: [0] }), 0.5600000023841858), "0.560000");
});

test("a u32 face's value is printed as the motor wrote it, never rounded into an integer", () => {
  // A non-integral value under a `u32` face would be a broken face; rounding it
  // here would print a number the engine never produced.
  assert.equal(formatValue(face({ kind: "u32", values: [0] }), 2.5), "2.5");
});

test("formatScore is the six-decimal form, and it is a score's form under either kind", () => {
  // `modularity` is an f64 whatever the face's kind says: a Louvain partition's
  // quality is not an integer because its community ids are.
  assert.equal(formatScore(0.3045267489711934), "0.304527");
  assert.equal(formatScore(0), "0.000000");
  assert.equal(formatScore(-1.5), "-1.500000");
});

test("valueAt is the node's own value, and null for an index past the face", () => {
  const result = face({ kind: "f64", values: [0.5, 0.25] });
  assert.equal(valueAt(result, 1), 0.25);
  assert.equal(valueAt(result, 2), null);
  assert.equal(valueAt(result, -1), null);
});

test("fillFor reads the fills by index and falls back to the style's own fill", () => {
  const fills = ["#111111", "#222222"];
  assert.equal(fillFor(fills, 1), "#222222");
  assert.equal(fillFor(fills, 5), null);
  assert.equal(fillFor(null, 0), null);
});

test("hoverValue is the node's own value, formatted by the face's kind", () => {
  assert.equal(hoverValue(face({ kind: "f64", values: [0.5, 0.25] }), 1), "0.250000");
  assert.equal(hoverValue(face({ kind: "u32", values: [3, 0] }), 0), "3");
});

test("hoverValue is null for an index the face does not cover", () => {
  const result = face({ kind: "u32", values: [0, 1] });
  assert.equal(hoverValue(result, 2), null);
  assert.equal(hoverValue(result, -1), null);
});

test("coversGraph accepts a face with one value per node", () => {
  assert.equal(coversGraph(face({ kind: "f64", values: [1, 2, 3] }), 3), true);
});

test("coversGraph refuses a face whose length is not the graph's", () => {
  // Values and nodeCount are both checked: a face that disagreed with itself
  // would colour the wrong nodes, and the panel says so rather than drawing it.
  assert.equal(coversGraph(face({ kind: "f64", values: [1, 2] }), 3), false);
  assert.equal(coversGraph({ id: "x", kind: "f64", nodeCount: 2, values: [1, 2, 3] }, 3), false);
  assert.equal(coversGraph(face({ kind: "f64", values: [] }), 0), true);
});

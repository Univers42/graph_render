import assert from "node:assert/strict";
import { test } from "node:test";

import { GROUP_PALETTE, MUTED, RAMP, rampOf } from "../src/look/palette.ts";
import { type LookInput, hiddenOf, legendOf, styleInputOf } from "../src/look/styleOf.ts";
import { metaOf } from "../src/source/meta.ts";
import { DEFAULT_SETTINGS } from "../src/state/settings.ts";
import { node } from "./support.ts";

const NODES = [
  node("a", { group: "Alpha", weight: 1, kind: "database", label: "Graph notes" }),
  node("b", { group: "Beta", weight: 0.5, kind: "note", label: "Layout design" }),
  node("c", { group: "Alpha", weight: 0.25, label: "Memory log" }),
  node("d", { weight: 0, label: "Orphan" }),
];
const META = metaOf(NODES, ["a", "b", "c", "d"], { source: Uint32Array.of(0, 0), target: Uint32Array.of(1, 2) });
const BASE: LookInput = {
  meta: META, appearance: DEFAULT_SETTINGS.appearance, filter: DEFAULT_SETTINGS.filter, analysis: null,
};

function look(patch: Partial<LookInput>): LookInput {
  return { ...BASE, ...patch };
}

test("the palettes are hex colours, and the ramp runs between its two ends", () => {
  for (const colour of [...GROUP_PALETTE, ...RAMP, MUTED]) assert.match(colour, /^#[0-9a-f]{6}$/);
  assert.equal(RAMP.length, 24);
  assert.deepEqual(rampOf(2), [RAMP[0], RAMP[23]]);
  assert.equal(new Set(RAMP).size, 24);
});

test("by default nodes are coloured by group and sized by weight", () => {
  const style = styleInputOf(BASE);
  assert.deepEqual(style.labels, ["Graph notes", "Layout design", "Memory log", "Orphan"]);
  assert.deepEqual([...style.colours], [0, 1, 0, 2]);
  assert.deepEqual(style.palette.slice(0, 3), [GROUP_PALETTE[0], GROUP_PALETTE[1], GROUP_PALETTE[2]]);
  assert.deepEqual([...style.weights], [1, 0.5, 0.25, 0]);
  assert.deepEqual(style.sizing, { base: 4, gain: 2.5 });
  assert.equal(style.hidden, null);
});

test("sizing by degree reads the link counts, and uniform keeps the rank but not the size", () => {
  const byDegree = styleInputOf(look({ appearance: { ...BASE.appearance, sizeBy: "degree" } }));
  assert.deepEqual([...byDegree.weights], [1, 0.5, 0.5, 0]);
  const uniform = styleInputOf(look({ appearance: { ...BASE.appearance, sizeBy: "uniform", nodeScale: 2 } }));
  assert.deepEqual([...uniform.weights], [1, 0.5, 0.25, 0]);
  assert.deepEqual(uniform.sizing, { base: 8, gain: 0 });
});

test("colouring by kind uses one colour per node kind", () => {
  const style = styleInputOf(look({ appearance: { ...BASE.appearance, colourBy: "kind" } }));
  assert.deepEqual([...style.colours], [2, 1, 0, 0]);
});

test("a score is coloured along the ramp and sizes the node", () => {
  const analysis = { id: "analysis.centrality.degree", kind: "f64" as const, values: Float64Array.of(10, 5, 0, 0) };
  const style = styleInputOf(look({ analysis, appearance: { ...BASE.appearance, colourBy: "analysis", sizeBy: "analysis" } }));
  assert.deepEqual(style.palette, RAMP);
  assert.deepEqual([...style.colours], [23, 12, 0, 0]);
  assert.deepEqual([...style.weights], [1, 0.5, 0, 0]);
});

test("a labelling is coloured by label, and equal scores do not divide by zero", () => {
  const labels = { id: "analysis.components.weak", kind: "u32" as const, values: Uint32Array.of(0, 0, 0, 11) };
  const style = styleInputOf(look({ analysis: labels, appearance: { ...BASE.appearance, colourBy: "analysis", sizeBy: "analysis" } }));
  assert.deepEqual([...style.colours], [0, 0, 0, 11 % GROUP_PALETTE.length]);
  const flat = { id: "x", kind: "f64" as const, values: Float64Array.of(3, 3, 3, 3) };
  const flatStyle = styleInputOf(look({ analysis: flat, appearance: { ...BASE.appearance, colourBy: "analysis", sizeBy: "analysis" } }));
  assert.deepEqual([...flatStyle.colours], [0, 0, 0, 0]);
  assert.deepEqual([...flatStyle.weights], [0, 0, 0, 0]);
});

test("colouring by an analysis that is not there falls back to one colour", () => {
  const style = styleInputOf(look({ appearance: { ...BASE.appearance, colourBy: "analysis" } }));
  assert.deepEqual([...style.colours], [0, 0, 0, 0]);
  assert.deepEqual(style.palette, [MUTED]);
});

test("filters hide by text, by group and by link count, and together", () => {
  const hide = (filter: Partial<LookInput["filter"]>): number[] | null => {
    const hidden = hiddenOf(META, { ...BASE.filter, ...filter });
    return hidden === null ? null : [...hidden];
  };
  assert.equal(hide({}), null);
  assert.deepEqual(hide({ text: "LOG" }), [1, 1, 0, 1]);
  assert.deepEqual(hide({ hiddenGroups: ["Alpha"] }), [1, 0, 1, 0]);
  assert.deepEqual(hide({ minDegree: 1 }), [0, 0, 0, 1]);
  assert.deepEqual(hide({ minDegree: 1, hiddenGroups: ["Beta"] }), [0, 1, 0, 1]);
  assert.deepEqual(hide({ text: "zzz" }), [1, 1, 1, 1]);
});

test("the legend counts what each colour stands for", () => {
  assert.deepEqual(legendOf(BASE), [
    { colour: GROUP_PALETTE[0], label: "Alpha", count: 2 },
    { colour: GROUP_PALETTE[1], label: "Beta", count: 1 },
    { colour: GROUP_PALETTE[2], label: "(no group)", count: 1 },
  ]);
  const scored = { id: "analysis.centrality.degree", kind: "f64" as const, values: Float64Array.of(10, 5, 0, 0) };
  assert.deepEqual(legendOf(look({ analysis: scored, appearance: { ...BASE.appearance, colourBy: "analysis" } })), [
    { colour: RAMP[0], label: "0", count: 2 },
    { colour: RAMP[23], label: "10", count: 1 },
  ]);
});

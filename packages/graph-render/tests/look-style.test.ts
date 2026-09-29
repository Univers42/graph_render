/**
 * The look style: one style whose palette is the colormap of the preset, whose labels are
 * the source's own rank -> declutter -> cut selection, and whose geometry comes from the
 * same preset. Every pin is exact, so a different operator or constant fails here.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_PRESET, LABEL_FONT_PX, LABEL_LIMIT, LOOKS, type Look } from "../src/look/presets.ts";
import { type LookStyleInput, lookStyle } from "../src/look/style.ts";
import { cssOf } from "../src/colour/srgb.ts";
import { sampleColormap } from "../src/colour/colormap.ts";

/** LOOKS is keyed by name, so the default is read through this and checked once. */
function pick(name: string): Look {
  const found = LOOKS[name];
  assert.ok(found, `no look named ${name}`);
  return found;
}

const LOOK = pick(DEFAULT_PRESET);

/** 0, 0.5, 0.25, 1: four distinct values, in no sorted order. */
const NORM = Float64Array.from([0, 0.5, 0.25, 1]);
const SCORES = Float64Array.from([1, 2, 3, 4]);
const TEXT = ["alpha", "be", "gamma", "d"];
/** Far apart, so the declutter keeps every one: half_w 0.30*26*len, half_h 0.62*26. */
const X = Float64Array.from([100, 400, 800, 1200]);
const Y = Float64Array.from([100, 100, 100, 100]);

function input(patch: Partial<LookStyleInput> = {}): LookStyleInput {
  return { look: LOOK, norm: NORM, scores: SCORES, text: TEXT, screenX: X, screenY: Y, ...patch };
}

/** `count` nodes in a row, far enough apart that no two label boxes can touch. */
function wide(count: number, scores: Float64Array): LookStyleInput {
  // A 3-character name estimates at 0.30*26*3 = 23.4 px of half-width, so twice that is
  // 46.8; six font sizes apart is well clear of it.
  const step = LABEL_FONT_PX * 6;
  return input({
    norm: Float64Array.from({ length: count }, (_, i) => i / (count - 1)),
    scores,
    text: Array.from({ length: count }, (_, i) => `n${i}`),
    screenX: Float64Array.from({ length: count }, (_, i) => 100 + i * step),
    screenY: new Float64Array(count),
  });
}

test("the palette is the preset's colormap, one entry per distinct normalised value", () => {
  const style = lookStyle(input());
  assert.equal(style.nodeCount, 4);
  assert.deepEqual(style.palette, [
    cssOf(sampleColormap("inferno", 0)),
    cssOf(sampleColormap("inferno", 0.5)),
    cssOf(sampleColormap("inferno", 0.25)),
    cssOf(sampleColormap("inferno", 1)),
  ]);
  // The buckets follow first-seen order, not the sorted order of the values.
  assert.deepEqual([...style.bucketStart], [0, 1, 2, 3, 4]);
  assert.deepEqual([...style.bucketItems], [0, 1, 2, 3]);
});

test("two nodes on the same colour share one palette entry", () => {
  const style = lookStyle(input({ norm: Float64Array.from([0.25, 0.25, 0.25, 1]) }));
  assert.equal(style.palette.length, 2);
  assert.deepEqual([...style.bucketStart], [0, 3, 4]);
  assert.deepEqual([...style.bucketItems], [0, 1, 2, 3]);
});

test("with no metric the palette is the preset's own flat node colour", () => {
  const style = lookStyle(input({ norm: null }));
  assert.deepEqual(style.palette, [cssOf(LOOK.node)]);
  assert.deepEqual([...style.bucketStart], [0, 4]);
});

test("every node is labelled when the boxes do not collide", () => {
  assert.deepEqual(lookStyle(input()).labels, TEXT);
});

test("a label whose estimated box overlaps one already kept is dropped (text_overlay.py:302-326)", () => {
  // Nodes 0 and 1 on the same point, and 0 ranks higher, so 1 is the one that goes.
  const style = lookStyle(input({
    scores: Float64Array.from([4, 2, 3, 1]),
    screenX: Float64Array.from([500, 500, 900, 1300]),
  }));
  assert.deepEqual(style.labels, ["alpha", "", "gamma", "d"]);
});

test("the cut to 18 comes after the declutter, on the ranking (executor.py:498-507)", () => {
  // 20 candidates, the last scoring highest: the cut keeps 19 down to 2.
  const scores = Float64Array.from({ length: 20 }, (_, i) => i);
  const style = lookStyle(wide(20, scores));
  assert.equal(LABEL_LIMIT, 18);
  // `labels` is indexed by node, so the kept set reads back in index order.
  assert.deepEqual(style.labels.filter((text) => text !== ""), [
    "n2", "n3", "n4", "n5", "n6", "n7", "n8", "n9",
    "n10", "n11", "n12", "n13", "n14", "n15", "n16", "n17", "n18", "n19",
  ]);
});

test("a tie in the scores is broken by index, so the cut drops the highest indices", () => {
  const style = lookStyle(wide(20, new Float64Array(20)));
  assert.deepEqual(style.labels.filter((text) => text !== ""), [
    "n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7", "n8",
    "n9", "n10", "n11", "n12", "n13", "n14", "n15", "n16", "n17",
  ]);
});

test("a candidate mask admits only the nodes it names, and the rest are never labelled", () => {
  const style = lookStyle(input({ candidates: Uint8Array.from([1, 0, 1, 0]) }));
  assert.deepEqual(style.labels, ["alpha", "", "gamma", ""]);
  // A mask of one node: the cut and the declutter see nothing else to choose from.
  assert.deepEqual(lookStyle(input({ candidates: Uint8Array.from([0, 0, 0, 1]) })).labels, ["", "", "", "d"]);
  assert.deepEqual(lookStyle(input({ candidates: new Uint8Array(4) })).labels, ["", "", "", ""]);
});

test("a blank text is never drawn, whatever the ranking says", () => {
  const style = lookStyle(input({ text: ["", "be", "", "d"] }));
  assert.deepEqual(style.labels, ["", "be", "", "d"]);
});

test("a SciGraphs label sits on its node, not below it", () => {
  assert.equal(lookStyle(input()).placement, "centred");
});

test("the weights are the colour coordinate, and the rank is heaviest first", () => {
  const style = lookStyle(input());
  assert.deepEqual([...style.weights], [0, 0.5, 0.25, 1]);
  assert.deepEqual([...style.rank], [3, 1, 2, 0]);
});

test("the edge width and the impostor bases are carried when the caller has them", () => {
  const style = lookStyle(input({ edgeWidth: 9.2 }));
  assert.equal(style.edgeWidth, 9.2);
  assert.equal(style.spheres, null);
  const bases = [sampleColormap("inferno", 0), sampleColormap("inferno", 1)];
  assert.deepEqual(lookStyle(input({ spheres: bases })).spheres, bases);
});

test("an empty sample is a valid style: no entry, no label", () => {
  const style = lookStyle(input({
    norm: new Float64Array(0), scores: new Float64Array(0), text: [],
    screenX: new Float64Array(0), screenY: new Float64Array(0),
  }));
  assert.equal(style.nodeCount, 0);
  assert.equal(style.placement, "centred");
  assert.deepEqual(style.labels, []);
});

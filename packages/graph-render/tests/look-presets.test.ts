/**
 * The look presets and the size constants, every literal pinned against the SciGraphs
 * sources that carry them: api/render.py for PRESETS and the pipeline example for the
 * gallery look and the label and fit constants.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Look } from "../src/look/presets.ts";
import {
  DEFAULT_PRESET,
  EDGE_WIDTH_REL,
  FIT_MARGIN,
  LABEL_BOX,
  LABEL_FONT_PX,
  LABEL_LIMIT,
  LABEL_PADDING_PX,
  LOOKS,
  LOOK_NAMES,
  NODE_RADIUS_REL,
  lookOf,
  worldRadius,
} from "../src/look/presets.ts";
import { COLORMAP_NAMES } from "../src/colour/tables.ts";
import { cssOf, srgbEncode } from "../src/colour/srgb.ts";

/** LOOKS is keyed by name, so every test goes through this to get one it can read. */
function pick(name: string): Look {
  const found = LOOKS[name];
  assert.ok(found, `no look named ${name}`);
  return found;
}

test("the six SciGraphs presets are there with gallery and scigraphs beside them (api/render.py:45-101)", () => {
  assert.deepEqual(LOOK_NAMES, [
    "slate", "paper", "ink", "blueprint", "terrain", "relief", "gallery", "scigraphs",
  ]);
  // The studio's default is the gallery look under the studio's own name, so the two
  // differ in nothing but the name that selects them.
  const { name: galleryName, ...gallery } = pick("gallery");
  assert.equal(galleryName, "gallery");
  assert.deepEqual(pick("scigraphs"), { name: "scigraphs", ...gallery });
  assert.equal(DEFAULT_PRESET, "scigraphs");
});

test("slate: background, colormap, color and edge_color of api/render.py:46-54", () => {
  const slate = pick("slate");
  assert.deepEqual(slate.background, [0.055, 0.06, 0.075]);
  assert.equal(slate.colormap, "viridis");
  assert.deepEqual(slate.node, [0.55, 0.6, 0.68]);
  assert.deepEqual(slate.edge, [0.16, 0.17, 0.21]);
});

test("paper: api/render.py:55-63", () => {
  const paper = pick("paper");
  assert.deepEqual(paper.background, [0.93, 0.93, 0.91]);
  assert.equal(paper.colormap, "plasma");
  assert.deepEqual(paper.node, [0.36, 0.39, 0.46]);
  assert.deepEqual(paper.edge, [0.22, 0.23, 0.26]);
});

test("ink: api/render.py:64-72", () => {
  const ink = pick("ink");
  assert.deepEqual(ink.background, [0.015, 0.015, 0.02]);
  assert.equal(ink.colormap, "turbo");
  assert.deepEqual(ink.node, [0.5, 0.52, 0.58]);
  assert.deepEqual(ink.edge, [0.13, 0.12, 0.15]);
});

test("blueprint: api/render.py:73-81", () => {
  const blueprint = pick("blueprint");
  assert.deepEqual(blueprint.background, [0.035, 0.065, 0.115]);
  assert.equal(blueprint.colormap, "cividis");
  assert.deepEqual(blueprint.node, [0.45, 0.56, 0.7]);
  assert.deepEqual(blueprint.edge, [0.14, 0.2, 0.3]);
});

test("terrain: api/render.py:82-90", () => {
  const terrain = pick("terrain");
  assert.deepEqual(terrain.background, [0.27, 0.25, 0.22]);
  assert.equal(terrain.colormap, "inferno");
  assert.deepEqual(terrain.node, [0.62, 0.58, 0.52]);
  assert.deepEqual(terrain.edge, [0.22, 0.2, 0.18]);
});

test("relief shares terrain's backdrop and colormap, api/render.py:91-100", () => {
  const relief = pick("relief");
  assert.deepEqual(relief.background, pick("terrain").background);
  assert.equal(relief.colormap, "inferno");
  assert.deepEqual(relief.node, [0.8, 0.76, 0.7]);
  assert.deepEqual(relief.edge, [0.26, 0.24, 0.21]);
});

test("every colormap named by a preset exists in the generated tables", () => {
  const known = new Set<string>(COLORMAP_NAMES);
  for (const name of LOOK_NAMES) {
    const entry = LOOKS[name];
    assert.ok(entry, name);
    assert.ok(known.has(entry.colormap), name);
  }
});

test(
  "gallery: the fig1/fig6 values of 05-reproducible-pipeline.qmd:121,132-137,139-145, and gallery alone",
  () => {
    const gallery = pick("gallery");
    assert.deepEqual(gallery.background, [0.035, 0.035, 0.045]);
    assert.equal(gallery.colormap, "inferno");
    assert.deepEqual(gallery.edge, [0.16, 0.16, 0.19]);
    // The flat node colour is slate's: the spec sets no flat colour, so the node
    // is coloured by the metric, and a look with no metric falls back to slate.
    assert.deepEqual(gallery.node, pick("slate").node);
    // Background, colormap and edge all differ from slate, so gallery is not
    // slate under another name.
    assert.notDeepEqual(gallery.background, pick("slate").background);
    assert.notEqual(gallery.colormap, pick("slate").colormap);
    assert.notDeepEqual(gallery.edge, pick("slate").edge);
  },
);

test("the gallery edge encodes to rgb(111, 111, 121)", () => {
  // 05-reproducible-pipeline.qmd:132-137 carries edge_base_color [0.16, 0.16, 0.19].
  assert.deepEqual(pick("gallery").edge, [0.16, 0.16, 0.19]);
  assert.equal(cssOf(pick("gallery").edge), "rgb(111, 111, 121)");
});

test("the gallery background encodes to the canvas rounding of (0.035, 0.035, 0.045)", () => {
  const expected = [0.035, 0.035, 0.045]
    .map((c) => Math.round(255 * srgbEncode(c)))
    .join(", ");
  assert.equal(cssOf(pick("gallery").background), `rgb(${expected})`);
});

test("the relative sizes come from 05-reproducible-pipeline.qmd:126-127", () => {
  assert.equal(NODE_RADIUS_REL, 0.022);
  // edge_radius_rel 0.0035 and the 2D stroke width is its diameter.
  assert.equal(EDGE_WIDTH_REL, 2 * 0.0035);
});

test("the label and fit constants come from 05-reproducible-pipeline.qmd:164,673,675,679", () => {
  assert.equal(FIT_MARGIN, 1.12);
  assert.equal(LABEL_LIMIT, 18);
  assert.equal(LABEL_FONT_PX, 26);
  assert.equal(LABEL_PADDING_PX, 3);
  assert.equal(LABEL_BOX, "rgba(0,0,0,0.6)");
});

test("worldRadius is half the diagonal of the box, floored at 1e-6 (executor.py:632,650)", () => {
  const bounds = { minX: -3, minY: -4, maxX: 3, maxY: 4 };
  assert.equal(worldRadius(bounds), 5);
  assert.equal(worldRadius({ minX: 0, minY: 0, maxX: 10, maxY: 0 }), 5);
  // A degenerate box is one node or coincident nodes, and every relative size
  // would be 0 without the floor the source applies.
  assert.equal(worldRadius({ minX: 1, minY: 1, maxX: 1, maxY: 1 }), 1e-6);
  assert.equal(worldRadius({ minX: 0, minY: 0, maxX: 1e-9, maxY: 0 }), 1e-6);
});

test("an unknown look name falls back to slate, as api/render.py:106-109 does", () => {
  assert.equal(lookOf("SLATE").name, "slate");
  assert.equal(lookOf("nope").name, "slate");
  assert.equal(lookOf("gallery").name, "gallery");
  // A name that is not a key but is an inherited property must fall back too.
  assert.equal(lookOf("constructor").name, "slate");
  assert.equal(lookOf("__proto__").name, "slate");
});

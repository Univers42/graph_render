/**
 * A preset turned into the painter's theme: the one place the SciGraphs linear colours
 * become CSS. Every value is pinned against the source it comes from.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Look } from "../src/look/presets.ts";
import { DEFAULT_PRESET, LOOKS, LOOK_NAMES, lookOf } from "../src/look/presets.ts";
import { lookTheme } from "../src/look/theme.ts";
import { DARK_THEME, LIGHT_THEME } from "../src/theme.ts";
import { LABEL_HEIGHT } from "../src/labels.ts";
import { cssOf } from "../src/colour/srgb.ts";

/** LOOKS is keyed by name, so every test goes through this to get one it can read. */
function pick(name: string): Look {
  const found = LOOKS[name];
  assert.ok(found, `no look named ${name}`);
  return found;
}

test("the default preset is the SciGraphs gallery look, under the studio's own name", () => {
  assert.equal(LOOK_NAMES.at(-1), "scigraphs");
  assert.equal(pick("scigraphs").name, "scigraphs");
  assert.deepEqual(lookOf(DEFAULT_PRESET).background, LOOKS.gallery?.background);
  assert.deepEqual(lookOf(DEFAULT_PRESET).colormap, LOOKS.gallery?.colormap);
  assert.deepEqual(lookOf(DEFAULT_PRESET).edge, LOOKS.gallery?.edge);
  // lookOf still falls back to slate, which is what api/render.py:106-109 does.
  assert.equal(lookOf("nope").name, "slate");
});

test("the background is the CSS of the linear triple, encoded exactly once", () => {
  for (const name of LOOK_NAMES) {
    assert.equal(lookTheme(pick(name)).background, cssOf(pick(name).background), name);
  }
  // 05-reproducible-pipeline.qmd:139-145: 255*encode = 52.526, 52.526, 59.873.
  assert.equal(lookTheme(pick("scigraphs")).background, "rgb(53, 53, 60)");
});

test("the edge colour is the preset's edge_color, opaque, as the tube is drawn opaque", () => {
  // 05-reproducible-pipeline.qmd:132-137 carries edge_base_color [0.16, 0.16, 0.19].
  const theme = lookTheme(pick("scigraphs"));
  assert.equal(theme.edge, "rgb(111, 111, 121)");
  assert.equal(theme.edgeLit, cssOf([0.55, 0.6, 0.68]));
});

test("the label is 26 px DejaVu Sans, white, on the 0.6 black box of the source", () => {
  const theme = lookTheme(pick("scigraphs"));
  assert.equal(theme.label, "#ffffff");
  assert.equal(theme.labelHalo, "rgba(0,0,0,0.6)");
  assert.deepEqual(theme.labelBox, { fill: "rgba(0,0,0,0.6)", padding: 3 });
  assert.equal(theme.labelFont, '26px "DejaVu Sans", sans-serif');
  // 26*1.2 line height plus 3 px of padding on each side, rounded up to whole pixels.
  assert.equal(theme.labelHeight, 38);
});

test("the ring is the flat node colour and the rim the background it is cut against", () => {
  const theme = lookTheme(pick("scigraphs"));
  assert.equal(theme.ring, "rgb(196, 203, 215)");
  assert.equal(theme.rim, theme.background);
  assert.equal(theme.dimAlpha, 0.16);
});

test("the studio's own themes keep the halo and the 16 px label box they had", () => {
  for (const theme of [DARK_THEME, LIGHT_THEME]) {
    assert.equal(theme.labelBox, null);
    assert.equal(theme.labelHeight, LABEL_HEIGHT);
  }
  assert.equal(DARK_THEME.background, "#1b1b1f");
  assert.equal(LIGHT_THEME.background, "#fbfbfc");
});

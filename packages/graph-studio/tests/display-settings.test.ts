// The display panel's members of the appearance: ranges, defaults, and the wire round trip.
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DEFAULT_SETTINGS, EDGE_COLOURS, EDGE_STYLES, LINK_THICKNESS, NODE_SCALE, SettingsRefusal, TEXT_FADE, THEMES,
  readSettings, withAppearance,
} from "../src/state/settings.ts";

test("the ranges are the plan's", () => {
  assert.deepEqual([NODE_SCALE.min, NODE_SCALE.max], [0.2, 5]);
  assert.deepEqual([LINK_THICKNESS.min, LINK_THICKNESS.max], [0.1, 5]);
  assert.deepEqual([TEXT_FADE.min, TEXT_FADE.max], [-3, 3]);
  assert.deepEqual(EDGE_STYLES, ["straight", "curve"]);
});

test("the themes include both obsidian ones and the SciGraphs presets", () => {
  for (const name of ["dark", "light", "obsidian-dark", "obsidian-light", "slate", "paper", "blueprint"]) {
    assert.ok(THEMES.includes(name), name);
  }
});

test("the defaults draw as before: no arrows, no glow, straight, neutral sliders", () => {
  const { appearance } = DEFAULT_SETTINGS;
  assert.deepEqual(
    [appearance.arrows, appearance.textFade, appearance.linkThickness, appearance.edgeStyle, appearance.glow, appearance.glowStrength],
    [false, 0, 1, "straight", false, 1],
  );
});

test("a display change survives JSON", () => {
  const next = withAppearance(DEFAULT_SETTINGS, {
    theme: "obsidian-dark", arrows: true, textFade: -2.5, linkThickness: 5, edgeStyle: "curve",
    edgeColour: "gradient", glow: true, glowStrength: 2, nodeScale: 0.2,
  });
  assert.deepEqual(readSettings(JSON.parse(JSON.stringify(next))), next);
});

test("the edge colour has two modes and the default is flat", () => {
  assert.deepEqual(EDGE_COLOURS, ["flat", "gradient"]);
  assert.equal(DEFAULT_SETTINGS.appearance.edgeColour, "flat");
  assert.equal(withAppearance(DEFAULT_SETTINGS, { edgeColour: "gradient" }).appearance.edgeColour, "gradient");
});

test("a value outside its range is refused, naming the member", () => {
  for (const [name, value] of [["textFade", 3.1], ["linkThickness", 0.05], ["nodeScale", 5.5], ["edgeStyle", "wavy"], ["edgeColour", "rainbow"]] as const) {
    const bad = { ...DEFAULT_SETTINGS, appearance: { ...DEFAULT_SETTINGS.appearance, [name]: value } };
    assert.throws(() => readSettings(JSON.parse(JSON.stringify(bad))), (e: unknown) =>
      e instanceof SettingsRefusal && e.message.includes(`appearance.${name}`), name);
  }
});

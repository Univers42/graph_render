// Every theme the studio offers resolves to a complete Theme, and the two obsidian ones are ours.
import assert from "node:assert/strict";
import { test } from "node:test";

import { LOOK_NAMES } from "../src/look/presets.ts";
import { THEME_NAMES, isLightTheme, themeNamed } from "../src/look/themes.ts";

test("the names are dark, light, both obsidian themes and every SciGraphs preset", () => {
  assert.deepEqual(THEME_NAMES.slice(0, 4), ["dark", "light", "obsidian-dark", "obsidian-light"]);
  assert.deepEqual(THEME_NAMES.slice(4), LOOK_NAMES);
});

test("every name has a theme with the label metrics set", () => {
  for (const name of THEME_NAMES) {
    const theme = themeNamed(name);
    assert.ok(theme.labelHeight > 0, name);
    assert.ok(theme.labelFont.length > 0, name);
    assert.match(theme.background, /^(#|rgb)/, name);
  }
});

test("the obsidian themes are pinned values of our own", () => {
  assert.equal(themeNamed("obsidian-dark").background, "#1e1e1e");
  assert.equal(themeNamed("obsidian-light").background, "#ffffff");
  assert.notEqual(themeNamed("obsidian-dark").ring, themeNamed("dark").ring);
});

test("an unknown name falls back to dark", () => {
  assert.equal(themeNamed("sepia"), themeNamed("dark"));
});

test("light is read off the background, not the name", () => {
  const lit = THEME_NAMES.filter(isLightTheme);
  assert.deepEqual(lit, ["light", "obsidian-light", "paper"]);
});

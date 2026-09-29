// The background modes as a theme: theme keeps the ground, flat and aurora replace it.
import assert from "node:assert/strict";
import { test } from "node:test";

import { backdropTheme } from "../src/look/backdrop.ts";
import { THEME_NAMES, isLightTheme, themeNamed } from "../src/look/themes.ts";

test("theme mode hands the theme back untouched", () => {
  for (const name of THEME_NAMES) {
    const theme = themeNamed(name);
    assert.equal(backdropTheme(theme, "theme", false), theme);
  }
});

test("flat and aurora set a backdrop and leave every other member alone", () => {
  const base = themeNamed("dark");
  const flat = backdropTheme(base, "flat", false);
  assert.deepEqual(flat.backdrop, { mode: "flat", colour: "#20232a" });
  assert.deepEqual({ ...flat, backdrop: undefined }, { ...base, backdrop: undefined });
  assert.equal(backdropTheme(base, "aurora", false).backdrop?.mode, "aurora");
});

test("the flat colour and the aurora's first stop differ from every theme ground", () => {
  for (const name of THEME_NAMES) {
    const light = isLightTheme(name);
    const theme = themeNamed(name);
    const flat = backdropTheme(theme, "flat", light).backdrop;
    const aurora = backdropTheme(theme, "aurora", light).backdrop;
    assert.notEqual(flat?.mode === "flat" ? flat.colour : "", theme.background, name);
    assert.notEqual(aurora?.mode === "aurora" ? aurora.stops[0] : "", theme.background, name);
    assert.notEqual(flat?.mode === "flat" ? flat.colour : "", aurora?.mode === "aurora" ? aurora.stops[0] : "", name);
  }
});

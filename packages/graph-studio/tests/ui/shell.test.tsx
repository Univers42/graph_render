// The chrome as a whole: the root the stylesheet hangs off, and what floats in it.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { withAppearance } from "../../src/state/settings.ts";
import { createPreviews } from "../../src/host/previews.ts";
import { Shell } from "../../src/ui/Shell.tsx";
import { DRAWN, fakeBar, fakeView, markup, studioWith } from "./desk.ts";

function shell(state = DRAWN): string {
  const { studio } = studioWith(state);
  return markup(createElement(Shell, { studio, view: fakeView(), keys: new EventTarget(), bar: fakeBar(), previews: createPreviews({ resolver: () => null }) }));
}

test("the root carries the theme the settings ask for", () => {
  assert.match(shell(DRAWN), /<div class="gs-chrome" data-theme="dark">/);
  const light = { ...DRAWN, settings: withAppearance(DRAWN.settings, { theme: "light" }) };
  assert.match(shell(light), /<div class="gs-chrome" data-theme="light">/);
});

test("the panels are the ones the layout names, and the console is not among them", () => {
  const html = shell();
  for (const panel of ["gs-left", "gs-search", "gs-dock", "gs-legend", "gs-hud", "gs-nav"]) {
    assert.ok(html.includes(panel), panel);
  }
  assert.ok(!html.includes("gs-console"), "the console opens only when it is asked for");
});

test("nothing floats over the graph until there is something to say", () => {
  const html = shell({ ...DRAWN, error: null, busy: [] });
  assert.ok(!html.includes("gs-toast"));
  assert.ok(!html.includes("gs-inspector"), "and nothing is selected");
});

// The dock: the sections in order, every action in one of them, and why one is refused.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { DOCK_SECTIONS, studioActions } from "../../src/actions/all.ts";
import { Dock, keepsShut } from "../../src/ui/Dock.tsx";
import { DRAWN, IDLE, fakeBar, markup, studioWith } from "./desk.ts";

function dock(state = DRAWN): string {
  const { studio } = studioWith(state);
  return markup(createElement(Dock, { studio, open: true, onToggle: () => undefined, bar: fakeBar() }));
}

test("the section titles stand in the order the sections are declared", () => {
  const html = dock();
  const at = DOCK_SECTIONS.map((name) => html.indexOf(`>${name}<`));
  assert.ok(at.every((where) => where > 0), "every section title is in the markup");
  assert.deepEqual([...at].sort((a, b) => a - b), at, "and they are in order");
});

test("every action that names a section has its title in the markup", () => {
  const html = dock();
  for (const action of studioActions()) {
    if (action.section === null) continue;
    assert.ok(html.includes(action.title), action.title);
  }
});

test("a header says whether its section is open, and points at the body it owns", () => {
  const html = dock();
  assert.match(html, /aria-expanded="true"[^>]*aria-controls="gs-dock-layout"/);
  assert.match(html, /aria-expanded="false"[^>]*aria-controls="gs-dock-export"/);
  assert.match(html, /id="gs-dock-source"[^>]*aria-labelledby="gs-dock-source-head"/);
});

test("the dock opens on the layout alone: the graph is what the page is for", () => {
  const html = dock();
  assert.equal(html.match(/aria-expanded="true"[^>]*aria-controls="gs-dock-(?!body)/g)?.length, 1);
  assert.match(html, /<div[^>]*id="gs-dock-source"[^>]*hidden=""/, "a closed section is hidden, not removed");
  assert.doesNotMatch(html, /<div[^>]*id="gs-dock-layout"[^>]*hidden/);
});

test("with nothing drawn the layout is refused, in place, with its reason", () => {
  const html = dock(IDLE);
  assert.match(html, /class="gs-reason">no graph is loaded</);
  const layout = html.slice(html.indexOf(">Layout<"), html.indexOf(">Edges<"));
  assert.match(layout, /disabled/, "every control of the layout is disabled");
  assert.ok(html.includes("Save the picture"), "a refused action stays visible");
});

test("the dock collapses as one, and the button that does it says so", () => {
  const html = dock();
  assert.match(html, /aria-expanded="true"[^>]*aria-controls="gs-dock-body"/);
  assert.match(html, />Controls</);
  assert.match(html, /id="gs-dock-body"/);
});

test("the Forces section shows nine labelled sliders, all aria-disabled, with the reason once", () => {
  const html = dock();
  const sliders = ["Center force", "Repel force", "Link force", "Link distance", "Node spacing", "Friction", "Cooling", "Repel range", "Accuracy"];
  for (const title of [...sliders, "Spread", "Compact", "Reset", "Animate", "Pause", "Resume"]) {
    assert.ok(html.includes(title), title);
  }
  assert.equal(html.match(/type="range"[^>]*aria-disabled="true"/g)?.length, sliders.length);
  assert.equal(html.split("live forces need the motor session (force-wasm)").length - 1, 1);
});

test("a shut section keeps what it drew; it redraws while open and the moment it opens", () => {
  const shut = { open: false, name: "Export" };
  assert.equal(keepsShut(shut, shut), true, "a store change under a shut section draws nothing");
  assert.equal(keepsShut(shut, { ...shut, open: true }), false, "opening it draws it from the state now");
  assert.equal(keepsShut({ ...shut, open: true }, { ...shut, open: true }), false, "an open one follows the state");
});

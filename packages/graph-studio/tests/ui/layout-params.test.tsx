/**
 * The Layout settings panel: one control per published parameter, of the kind the schema says
 * it is, in the dock's own styles, with the reset beside it.
 *
 * `renderToStaticMarkup` runs no effect, so what a commit does is not observable here: the one
 * run per frame a commit is collapsed into is what `one-run.test.ts` holds, against a
 * scheduler the test drives itself, and the run it asks for is what `params-actions.test.ts`
 * holds over a desk.
 */
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import type { StudioState } from "../../src/state/model.ts";
import { Dock } from "../../src/ui/Dock.tsx";
import { LayoutParamsPanel } from "../../src/ui/LayoutParamsPanel.tsx";
import { DRAWN, DRAWN_LAYOUT, DRAWN_SCHEMA, fakeBar, markup, studioWith } from "./desk.ts";

function panel(state: StudioState = DRAWN): string {
  const { studio } = studioWith(state);
  return markup(createElement(LayoutParamsPanel, { studio, state }));
}

const FLOATS = DRAWN_SCHEMA.filter((spec) => spec.kind === "float");
const INTS = DRAWN_SCHEMA.filter((spec) => spec.kind === "int");

test("one control per published parameter, labelled with the motor's own names", () => {
  const html = panel();
  for (const spec of DRAWN_SCHEMA) assert.ok(html.includes(`>${spec.name}<`), spec.name);
  assert.equal(html.match(/class="gs-field"/g)?.length, DRAWN_SCHEMA.length);
  assert.ok(html.includes(DRAWN_LAYOUT), "the panel says which layout it is showing");
});

test("a float is a slider and an int a number field, each with the bounds the schema published", () => {
  const html = panel();
  const ranges = [...html.matchAll(/class="gs-range"[^>]*min="([^"]*)" max="([^"]*)" step="([^"]*)"/g)];
  assert.deepEqual(ranges.map(([, min, max, step]) => [min, max, step]),
    FLOATS.map((spec) => [String(spec.min), String(spec.max), String(spec.step)]));
  const numbers = [...html.matchAll(/type="number"[^>]*min="([^"]*)" max="([^"]*)"/g)];
  assert.equal(numbers.length, INTS.length);
  assert.deepEqual(numbers.map(([, min, max]) => [min, max]), INTS.map((spec) => [String(spec.min), String(spec.max)]));
});

test("a bool is a switch, not a number field", () => {
  const html = panel({
    ...DRAWN,
    schemas: { [DRAWN_LAYOUT]: [{ name: "arrows", kind: "bool", min: 0, max: 1, default: 0, step: 1, doc: "" }] },
  });
  assert.ok(html.includes('role="switch"'), html.slice(0, 300));
  assert.ok(!html.includes('type="number"'));
});

test("the value in force is what the state holds, and the motor's default where it holds none", () => {
  const html = panel();
  assert.ok(html.includes('value="120"'), "the value the user moved");
  assert.ok(html.includes('value="0.0001"'), "and the default of the one they did not");
});

test("with nothing published the panel says so and shows no control", () => {
  const quiet = "layout.force.barnes_hut";
  const html = panel({ ...DRAWN, settings: { ...DRAWN.settings, layout: quiet }, schemas: { [quiet]: [] } });
  assert.match(html, /class="gs-reason">layout\.force\.barnes_hut publishes no parameters/);
  assert.ok(!html.includes('class="gs-field"'));
});

test("with no graph the panel is refused in place, and every control is disabled", () => {
  const state = { ...DRAWN, graph: null };
  const html = panel(state);
  assert.match(html, /class="gs-reason">no graph is loaded/);
  assert.ok(html.includes("disabled"), "every control is disabled");
});

test("the dock's Layout settings section holds these controls and the way back to the defaults", () => {
  const { studio } = studioWith(DRAWN);
  const html = markup(createElement(Dock, { studio, open: true, onToggle: () => undefined, bar: fakeBar() }));
  const section = html.slice(html.indexOf(">Layout settings<"), html.indexOf(">Edges<"));
  for (const spec of DRAWN_SCHEMA) assert.ok(section.includes(`>${spec.name}<`), spec.name);
  assert.ok(section.includes("Back to the published defaults"), "the reset is in this section");
  assert.ok(section.includes('id="gs-dock-layout-settings"'), section.slice(0, 200));
});

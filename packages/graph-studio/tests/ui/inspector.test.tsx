// The inspector: what the selected node is, and what it is joined to.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { Inspector } from "../../src/ui/Inspector.tsx";
import { DRAWN, fakeView, markup, studioWith } from "./desk.ts";

function inspector(state = DRAWN): string {
  const { studio } = studioWith(state);
  return markup(createElement(Inspector, { studio, state, view: fakeView() }));
}

test("with nothing selected there is no inspector", () => {
  assert.equal(inspector({ ...DRAWN, selected: -1 }), "");
  assert.equal(inspector({ ...DRAWN, selected: 0, meta: null }), "");
});

test("the node is named, and what the drawing knows about it is listed", () => {
  const html = inspector({ ...DRAWN, selected: 0 });
  assert.match(html, /gs-title">Alpha</, "the label is the heading");
  assert.ok(html.includes(">a<"), "the id is there too");
  assert.ok(html.includes(">red<"), "and its group");
  assert.match(html, /Degree<\/span>\s*<span class="gs-kv-value">1</);
  assert.ok(html.includes(">record<"), "and its kind");
  assert.match(html, /aria-label="Close the inspector"/);
});

test("a node with an analysis beside it shows the value the analysis gave it", () => {
  const analysis = {
    id: "analysis.depth.bfs", kind: "f64" as const, values: Float64Array.of(1, 4, 9),
    converged: true, modularity: 0.5, max: 9, ms: 2,
  };
  const html = inspector({ ...DRAWN, selected: 1, analysis });
  assert.match(html, /depth\.bfs<\/span>\s*<span class="gs-kv-value">4</);
});

test("an analysis that does not cover every node says nothing about this one", () => {
  const analysis = {
    id: "analysis.depth.bfs", kind: "f64" as const, values: Float64Array.of(1, 4),
    converged: true, modularity: null, max: 4, ms: 1,
  };
  const html = inspector({ ...DRAWN, selected: 2, analysis });
  assert.ok(!html.includes("depth.bfs"));
});

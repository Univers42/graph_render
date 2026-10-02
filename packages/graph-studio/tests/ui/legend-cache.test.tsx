// The legend's counts and colour rows, and the one cache that keeps them off every render.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { Legend, type LegendView, legendViewOf } from "../../src/ui/Legend.tsx";
import type { GraphMeta } from "../../src/source/meta.ts";
import { metaOf } from "../../src/source/meta.ts";
import { type Group, type Settings, withAppearance, withGroups } from "../../src/state/settings.ts";
import { DRAWN, META, markup } from "./desk.ts";
import { node } from "../support.ts";

const GROUPS: readonly Group[] = [
  { name: "servers", query: "kind:server", colour: "#ff0055" },
  { name: "alpha", query: "alpha", colour: "#00aa88" },
  { name: "strays", query: "degree:0", colour: "#123456" },
];

/** A settings document with these groups, frozen the way the studio freezes one. */
function grouped(groups: readonly Group[] = GROUPS): Settings {
  return withGroups(DRAWN.settings, groups);
}

function legend(settings: Settings): string {
  return markup(createElement(Legend, { meta: META, settings, analysis: null }));
}

/** The cached view the panel draws: a graph is drawn, so there is always one to read. */
function viewOf(meta: GraphMeta, settings: Settings): LegendView {
  const made = legendViewOf({ meta, settings, analysis: null });
  assert.ok(made !== null, "a drawn graph has a legend to read");
  return made;
}

function view(settings: Settings): LegendView {
  return viewOf(META, settings);
}

/** The same three nodes as META, built again: equal in every field, a different object. */
function sameGraphAgain(): GraphMeta {
  return metaOf(
    [node("a", { label: "Alpha", group: "red" }), node("b", { label: "Beta", group: "red" })],
    ["a", "b"],
    { source: Uint32Array.of(0), target: Uint32Array.of(1) },
  );
}

test("with nothing drawn there is no legend", () => {
  assert.equal(markup(createElement(Legend, { meta: null, settings: DRAWN.settings, analysis: null })), "");
  assert.equal(legendViewOf({ meta: null, settings: grouped(), analysis: null }), null);
});

test("each group says how many nodes its own query matches", () => {
  const html = legend(grouped());
  // The three test nodes are Alpha, Beta (group red) and Gamma (group blue), each with a link.
  assert.match(html, /servers<\/span>\s*<span class="gs-count">0</, "a query no node matches counts zero");
  assert.match(html, /alpha<\/span>\s*<span class="gs-count">1</, "one node is named Alpha");
  assert.match(html, /strays<\/span>\s*<span class="gs-count">0</, "and no node is unlinked");
  assert.deepEqual(view(grouped()).counts, [0, 1, 0], "one count per group, in the order they are in");
});

test("the colour rows below the groups are the ones the drawing falls back on", () => {
  const html = legend(grouped());
  assert.match(html, /red<\/span>\s*<span class="gs-count">2</);
  assert.match(html, /blue<\/span>\s*<span class="gs-count">1</);
  assert.equal(html.match(/class="gs-legend-row"/g)?.length, 5, "three groups and two colours");
  assert.deepEqual(view(grouped()).rows.map((row) => row.label), ["red", "blue"]);
});

test("a query the grammar refuses counts zero rather than dropping the row or throwing", () => {
  const broken = [{ name: "broken", query: "colour:red", colour: "#654321" }];
  const html = legend(grouped(broken));
  assert.match(html, /broken<\/span>\s*<span class="gs-count">0</, "the row is on screen and says zero");
  assert.equal(html.match(/class="gs-legend-row"/g)?.length, 3, "not dropped, and nothing added beside it");
  assert.deepEqual(view(grouped(broken)).counts, [0], "and the count the panel reads says the same");
});

test("the same graph and the same groups hand back the same view, render after render", () => {
  const settings = grouped();
  const first = view(settings);
  const html = legend(settings);
  assert.equal(legend(settings), html, "the same graph draws the same legend");
  assert.equal(legendViewOf({ meta: META, settings, analysis: null }), first, "a fresh view each render is the cost this cache is here to remove");
  assert.equal(legendViewOf({ meta: META, settings: DRAWN.settings, analysis: null })?.counts.length, 0);
});

test("different groups or a different look give a different view, so nothing goes stale", () => {
  const settings = grouped();
  const base = view(settings);
  // The groups stay the very same array below, so only the appearance can tell these two apart.
  const recoloured = view(withAppearance(settings, { colourBy: "kind" }));
  assert.notEqual(recoloured, base, "the colour rows are read off the appearance as well");
  assert.deepEqual(recoloured.rows.map((row) => row.label), ["record"], "kind, not group, names the rows now");
  const fewer = view(grouped([{ name: "alpha", query: "alpha", colour: "#00aa88" }]));
  assert.notEqual(fewer, base, "the groups are half of what the view is keyed on");
  assert.deepEqual(fewer.counts, [1], "one count, not the three of the groups before it");
});

test("a graph equal to the cached one but not the same object is walked again", () => {
  const other = sameGraphAgain();
  const settings = grouped();
  const first = viewOf(other, settings);
  assert.notEqual(first, view(grouped()), "the cache is keyed on the identity of the graph, not on its fields");
  assert.deepEqual(first.counts, [0, 1, 0], "and the new graph is read for itself");
  assert.equal(viewOf(other, settings), first, "but a graph held still under one document is read once");
});
// What the colours on the canvas stand for, and the search that finds a node by name.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { Legend } from "../../src/ui/Legend.tsx";
import { Search } from "../../src/ui/Search.tsx";
import { allMatchesOf } from "../../src/ui/matches.ts";
import type { Group } from "../../src/state/settings.ts";
import type { StudioState } from "../../src/state/model.ts";
import { DRAWN, IDLE, META, markup, studioWith } from "./desk.ts";

const GROUPS: readonly Group[] = [
  { name: "servers", query: "kind:server", colour: "#ff0055" },
  { name: "alpha", query: "alpha", colour: "#00aa88" },
  { name: "strays", query: "degree:0", colour: "#123456" },
];

function grouped(state: StudioState): StudioState {
  return groupedWith(GROUPS, state);
}

function groupedWith(groups: readonly Group[], state: StudioState = DRAWN): StudioState {
  return { ...state, settings: { ...state.settings, groups } };
}

/** The legend is handed the three slices it draws, never the state around them. */
function legend(state: StudioState): string {
  return markup(createElement(Legend, { meta: state.meta, settings: state.settings, analysis: state.analysis }));
}

/** Where a label sits in the markup, so the rows can be compared in the order they render. */
function rowOf(html: string, label: string): number {
  return html.indexOf(`>${label}</span>`);
}

test("with nothing drawn there is no legend", () => {
  assert.equal(legend(IDLE), "");
});

test("one row per colour in use, with how many nodes wear it", () => {
  const html = legend(DRAWN);
  assert.equal(html.match(/class="gs-legend-row"/g)?.length, 2, "one row per group");
  assert.match(html, /red<\/span>\s*<span class="gs-count">2</);
  assert.match(html, /blue<\/span>\s*<span class="gs-count">1</);
  assert.match(html, /class="gs-swatch" style="background:#[0-9a-f]{6}"/, "each row carries its colour");
});

test("the named groups come first and in order, each wearing its own colour", () => {
  const html = legend(grouped(DRAWN));
  const swatches = html.match(/class="gs-swatch" style="background:#[0-9a-f]{6}"/g) ?? [];
  assert.deepEqual(
    swatches.slice(0, GROUPS.length),
    GROUPS.map((group) => `class="gs-swatch" style="background:${group.colour}"`),
    "the swatches carry the colour the studio was given, in the order the groups are in",
  );
  const order = [rowOf(html, "servers"), rowOf(html, "alpha"), rowOf(html, "strays")];
  assert.deepEqual([...order].sort((a, b) => a - b), order, "the labels keep the groups' own order");
  assert.ok(rowOf(html, "strays") < rowOf(html, "red"), "and the colouring rows follow them");
  assert.equal(html.match(/class="gs-legend-row"/g)?.length, 5, "three groups and two colours");
});

test("each group says how many nodes its own query matches", () => {
  const html = legend(grouped(DRAWN));
  // The three test nodes are Alpha, Beta (group red) and Gamma (group blue), each with a link.
  assert.match(html, /servers<\/span>\s*<span class="gs-count">0</, "a query no node matches counts zero");
  assert.match(html, /alpha<\/span>\s*<span class="gs-count">1</, "one node is named Alpha");
  assert.match(html, /strays<\/span>\s*<span class="gs-count">0</, "and no node is unlinked");
  const counted = legend(groupedWith([{ name: "first", query: "alpha", colour: "#111111" }]));
  assert.match(counted, /first<\/span>\s*<span class="gs-count">1</);
});

test("the search is one labelled input, and offers nothing until it is typed in", () => {
  const { studio } = studioWith(DRAWN);
  const html = markup(createElement(Search, { studio, meta: META, text: "", inputRef: { current: null } }));
  assert.match(html, /aria-label="Search nodes"/);
  assert.match(html, /placeholder="Search \( \/ \)"/, "an empty box says what it is, and the key that reaches it");
  assert.equal(html.match(/class="gs-result"/g), null, "no results for an empty query");
  assert.equal(html.match(/<input/g)?.length, 1);
});

test("the full list of matches is the ranking with no limit on it", () => {
  const labels = ["Alpha", "beta", "Alphabet", "Gamma", "alphabet soup", "delta"];
  assert.deepEqual(allMatchesOf(labels, "alpha"), [0, 2, 4], "prefixes first, then what holds the text");
  assert.deepEqual(allMatchesOf(labels, ""), [], "an empty text matches nothing");
  const many = Array.from({ length: 30 }, (_, i) => `node ${i}`);
  assert.equal(allMatchesOf(many, "node").length, 30, "what the mask is made of is not cut at eight");
});

test("a fit-to-results button dispatches the action it names, through the studio", () => {
  const { studio, seen } = studioWith(DRAWN);
  const html = markup(createElement(Search, { studio, meta: META, text: "", inputRef: { current: null } }));
  assert.match(html, /aria-label="Fit to results"/, "the control is named for a reader");
  assert.ok(html.includes(">fit to results</button>"), "and carries a visible label");
  // The registry must carry the ids the box dispatches; `search.fit` lives in the groups
  // action list, so this fails until `studioActions()` spreads GROUP_ACTIONS in.
  assert.ok(studio.registry.find("search") !== undefined, "`search` is how the box is typed into");
  assert.ok(studio.registry.find("search.fit") !== undefined, "`search.fit` is an action of the studio");
  assert.deepEqual(seen.cameras, [], "rendering the box moves nothing on its own");
});

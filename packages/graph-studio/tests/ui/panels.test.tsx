// What the colours on the canvas stand for, and the search that finds a node by name.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { Legend } from "../../src/ui/Legend.tsx";
import { Search } from "../../src/ui/Search.tsx";
import { DRAWN, IDLE, META, markup, studioWith } from "./desk.ts";

test("with nothing drawn there is no legend", () => {
  assert.equal(markup(createElement(Legend, { state: IDLE })), "");
});

test("one row per colour in use, with how many nodes wear it", () => {
  const html = markup(createElement(Legend, { state: DRAWN }));
  assert.equal(html.match(/class="gs-legend-row"/g)?.length, 2, "one row per group");
  assert.match(html, /red<\/span>\s*<span class="gs-count">2</);
  assert.match(html, /blue<\/span>\s*<span class="gs-count">1</);
  assert.match(html, /class="gs-swatch" style="background:#[0-9a-f]{6}"/, "each row carries its colour");
});

test("the search is one labelled input, and offers nothing until it is typed in", () => {
  const { studio } = studioWith(DRAWN);
  const html = markup(createElement(Search, { studio, meta: META, inputRef: { current: null } }));
  assert.match(html, /aria-label="Search nodes"/);
  assert.equal(html.match(/class="gs-result"/g), null, "no results for an empty query");
  assert.equal(html.match(/<input/g)?.length, 1);
});

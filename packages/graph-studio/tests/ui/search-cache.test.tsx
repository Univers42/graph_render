// The search over a large graph: the ranking must not move, and the work of lowercasing every
// label must happen once per graph rather than once per keystroke and per render.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { Search } from "../../src/ui/Search.tsx";
import { allMatchesOf, loweredLabelsOf, matchesOf } from "../../src/ui/matches.ts";
import { metaOf, type GraphMeta } from "../../src/source/meta.ts";
import { node } from "../support.ts";
import { DRAWN, IDLE, META, markup, studioWith } from "./desk.ts";

/** The box as the chrome draws it: one render, the html it produced. */
function box(text: string, meta: GraphMeta | null = META): string {
  const { studio } = studioWith(DRAWN);
  return markup(createElement(Search, { studio, meta, text, inputRef: { current: null } }));
}

/** The labels the box offers, in the order it renders them. */
function offered(html: string): readonly string[] {
  return [...html.matchAll(/class="gs-btn gs-result"[^>]*>([^<]*)</g)].map((row) => row[1] ?? "");
}

/** A graph of the size that used to be slow: the ranking must not depend on the size. */
function wideLabels(count: number): readonly string[] {
  return Array.from({ length: count }, (_, i) => `node ${i} ALPHA`);
}

/** A second graph, so the box can be asked about one `meta` and then another. */
function otherMeta(): GraphMeta {
  const nodes = [node("p", { label: "Alpha prime" })];
  return metaOf(nodes, ["p"], { source: Uint32Array.of(0), target: Uint32Array.of(0) });
}

test("the ranking is the one it was: prefixes first, then what holds the text", () => {
  const labels = ["Alpha", "beta", "Alphabet", "Gamma", "alphabet soup", "delta"];
  assert.deepEqual(loweredLabelsOf(labels), ["alpha", "beta", "alphabet", "gamma", "alphabet soup", "delta"]);
  assert.deepEqual(allMatchesOf(labels, "alpha"), [0, 2, 4]);
  assert.deepEqual(allMatchesOf(labels, "ALPHA"), [0, 2, 4]);
  assert.deepEqual(allMatchesOf(labels, ""), [], "an empty text matches nothing");
  assert.deepEqual(matchesOf(labels, "a", 2), [0, 2], "the cut is made after the whole list is ranked");
});

test("the index is built once per labels array, and the same one is handed out after", () => {
  const labels = wideLabels(4000);
  const first = loweredLabelsOf(labels);
  assert.equal(loweredLabelsOf(labels), first, "the same labels, the same index: a fresh one per keystroke would be the bug");
  assert.deepEqual(allMatchesOf(labels, "alpha"), Array.from({ length: 4000 }, (_, i) => i));
});

test("an equal but new labels array is a new graph, and is indexed of its own", () => {
  const labels = wideLabels(4);
  const rebuilt = [...labels];
  assert.notEqual(loweredLabelsOf(rebuilt), loweredLabelsOf(labels), "identity is the key, so a copy is rebuilt");
  assert.deepEqual(loweredLabelsOf(rebuilt), loweredLabelsOf(labels), "and what it holds is the same");
  assert.deepEqual(allMatchesOf(rebuilt, "node 2"), [2], "the copy ranks as itself, not as the array it came from");
});

test("a keystroke re-ranks against the one index, with nothing carried over", () => {
  const labels = wideLabels(50);
  const index = loweredLabelsOf(labels);
  assert.deepEqual(matchesOf(labels, "n", 3), [0, 1, 2]);
  assert.deepEqual(allMatchesOf(labels, "node 7"), [7], "what the first text matched is not offered to the next");
  assert.deepEqual(allMatchesOf(labels, "alpha"), Array.from({ length: 50 }, (_, i) => i));
  assert.equal(loweredLabelsOf(labels), index, "the index outlives every keystroke");
});

test("a second graph is searched out of its own labels, not the ones before it", () => {
  const other = otherMeta();
  assert.notEqual(other.labels, META.labels, "a new `meta` means a new array to key on");
  assert.deepEqual(offered(box("a", other)), ["Alpha prime"], "the new graph answers for itself");
  assert.deepEqual(offered(box("a", META)), ["Alpha", "Beta", "Gamma"], "and the first still answers for itself");
  assert.deepEqual(offered(box("a", other)), ["Alpha prime"], "going back does not serve the other index");
});

test("the box is one labelled input, and offers nothing until it is typed in", () => {
  const html = box("");
  assert.match(html, /aria-label="Search nodes"/);
  assert.match(html, /placeholder="Search \( \/ \)"/, "an empty box says what it is, and the key that reaches it");
  assert.equal(html.match(/class="gs-result"/g), null, "no results for an empty text");
  assert.equal(html.match(/<input/g)?.length, 1);
});

test("a typed text offers its matches and counts them all, not only the eight on screen", () => {
  const html = box("a");
  assert.deepEqual(offered(html), ["Alpha", "Beta", "Gamma"], "every node holds an a, in index order");
  assert.match(html, /Fit the camera to the 3 results/, "and the fit button counts the whole list");
  assert.match(html, /aria-label="Fit to results"/);
  assert.ok(html.includes(">fit to results</button>"));
});

test("each keystroke moves the list, and the old list is not left behind", () => {
  assert.deepEqual(offered(box("lph")), ["Alpha"], "one match narrows the box");
  assert.deepEqual(offered(box("a")), ["Alpha", "Beta", "Gamma"], "and a wider text brings the rest back");
  assert.deepEqual(offered(box("gamma")), ["Gamma"], "the list is the whole of what the text holds");
  assert.deepEqual(offered(box("z")), [], "a text nothing holds leaves no result standing");
  assert.deepEqual(offered(box("a")), offered(box("a")), "the same text renders the same list again");
});

test("before anything is drawn there is a box with nothing to offer", () => {
  const html = box("", null);
  assert.equal(html.match(/class="gs-result"/g), null);
  assert.match(html, /Fit the camera to the 0 results/);
  assert.equal(markup(createElement(Search, {
    studio: studioWith(IDLE).studio, meta: IDLE.meta, text: "a", inputRef: { current: null },
  })).match(/class="gs-result"/g), null, "no meta means no labels means no result");
});
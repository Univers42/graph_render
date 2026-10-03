/**
 * The data path of a reveal step: what it costs is what changed. A reveal changes `reveal`
 * and nothing else, so the colours, the palette and the weights the step did not touch must
 * come back as the same arrays, and only the revealed rows of the hidden mask may move.
 *
 * The identity assertions are the point. A `===` on the arrays is what a memo buys, and what
 * makes the step O(revealed rows) instead of O(nodes); a deep-equal would pass on a path that
 * rebuilt everything to exactly the same numbers, which is the cost this file exists to catch.
 *
 * Five nodes in ingest order, degrees 1, 2, 2, 2, 1, so `minDegree: 2` hides the first and the
 * last and the middle three read as drawn. A reveal shows node `i` while `i < shown`, so a step
 * from 2 to 3 is the smallest one that moves exactly one row: row 2 goes from hidden to drawn.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { StyleInput } from "../../graph-render/src/style.ts";
import { styleInputOf } from "../src/look/styleOf.ts";
import type { GraphMeta } from "../src/source/meta.ts";
import { metaOf } from "../src/source/meta.ts";
import { type Appearance, type Filter, type Group, DEFAULT_SETTINGS, withAppearance, withFilter, withGroups } from "../src/state/settings.ts";
import { desk, scriptedClient } from "./desk.ts";
import { node } from "./support.ts";

const NODES = [
  node("a", { group: "Alpha", weight: 1, kind: "database", label: "Alpha" }),
  node("b", { group: "Beta", weight: 1, kind: "note", label: "Beta" }),
  node("c", { group: "Alpha", weight: 1, kind: "note", label: "Gamma" }),
  node("d", { group: "Beta", weight: 1, kind: "tag", label: "Delta" }),
  node("e", { group: "Alpha", weight: 1, kind: "note", label: "Epsilon" }),
];
const META: GraphMeta = metaOf(NODES, ["a", "b", "c", "d", "e"], {
  source: Uint32Array.of(0, 1, 2, 3),
  target: Uint32Array.of(1, 2, 3, 4),
});

/** `sizeBy: "degree"` is the case the memo has to earn: the default `weight` hands back
 *  `meta.weight` itself and so is already the same array with no memoisations at all. */
function appearanceWith(patch: Partial<Appearance> = {}): Appearance {
  return withAppearance(DEFAULT_SETTINGS, { sizeBy: "degree", ...patch }).appearance;
}

function filterWith(patch: Partial<Filter>): Filter {
  return withFilter(DEFAULT_SETTINGS, patch).filter;
}

const APPEARANCE = appearanceWith();
const BY_DEGREE = filterWith({ minDegree: 2 });
const BY_TEXT = filterWith({ text: "lph" });
const NO_GROUPS: readonly Group[] = [];
const GROUPS: readonly Group[] = withGroups(DEFAULT_SETTINGS, [
  { name: "Beta", query: "kind:note", colour: "#f2a65a" },
]).groups;

/**
 * The memos key on the identity of `groups` and `filter`, as the studio's own frozen documents
 * do (`state/settings.ts`). So the empty list here is one array, not a fresh `[]` per call: a
 * test that built one per call would miss every memo and never see what it is asserting.
 */
function look(reveal: number | null, appearance = APPEARANCE, filter = BY_DEGREE, groups = NO_GROUPS): StyleInput {
  return styleInputOf({ meta: META, appearance, filter, groups, analysis: null, reveal });
}

/** The hidden mask, or the test stops: a null mask means "draw all", which cannot be compared. */
function maskOf(style: StyleInput): Uint8Array {
  const hidden = style.hidden;
  if (hidden === null || hidden === undefined) throw new Error("the style carries no hidden mask");
  return hidden;
}

test("a reveal step hands back the colours and the weights it did not change", () => {
  const before = look(2);
  const after = look(3);
  assert.ok(after.colours === before.colours, "the colours are the same array, not an equal one");
  assert.ok(after.palette === before.palette, "and the palette with them");
  assert.ok(after.weights === before.weights, "the weights are the same array too");
});

test("a reveal step moves only the rows it reveals", () => {
  const before = look(2);
  const after = look(3);
  const was = maskOf(before);
  const now = maskOf(after);
  assert.ok(!(now === was), "the mask itself is a new array: the filter's is never written to");
  assert.deepEqual([...maskOf(look(null))], [1, 0, 0, 0, 1], "the filter hides the two degree-1 nodes");
  assert.deepEqual([...was], [1, 0, 1, 1, 1], "at two revealed, rows 2 and up are still hidden");
  assert.deepEqual([...now], [1, 0, 0, 1, 1], "at three, row 2 is the one that came out");
  for (const row of [0, 1, 3, 4]) {
    assert.equal(now[row], was[row], `row ${row} is not revealed by this step and did not move`);
  }
});

test("showing every node and showing none are steps like any other", () => {
  assert.deepEqual([...maskOf(look(0))], [1, 1, 1, 1, 1], "reveal(0) has drawn nothing");
  const shown = look(3);
  const all = look(null);
  assert.ok(all.colours === shown.colours, "dropping the reveal does not rebuild the colours");
  assert.ok(all.weights === shown.weights, "and it does not rebuild the weights either");
  // Nothing to reveal is the one case where the style carries the filter's own mask rather than a
  // copy of it, so this is where the held mask can be read back off two calls.
  assert.ok(all.hidden === look(null).hidden, "and with nothing to reveal the mask is not rebuilt");
  assert.ok(!(shown.hidden === all.hidden), "while a revealing step copies it, as it must");
});

/**
 * The negative control, and the risk these memoisations carry: a memo that forgets one of its
 * keys returns a stale array rather than an equal one, and the drawing goes quietly wrong
 * instead of slow. Every key a part does not read must leave the other parts alone.
 *
 * Each pair re-reads `base` first. A memo holds one entry, so a lookup that asks for the colours
 * twice with a different `colourBy` between them is two misses by design (`look/memo.ts`); what
 * is asserted here is that one changed key misses and one unchanged key hits, not that a memo
 * remembers more than the last thing it was asked.
 */
test("each memo misses on its own keys and not on the others", () => {
  const first = look(2);
  const recoloured = look(2, appearanceWith({ colourBy: "kind" }));
  assert.ok(!(recoloured.colours === first.colours), "colourBy changed, so the colours are new");
  assert.ok(recoloured.weights === first.weights, "and the weights, which did not, are not");

  const second = look(2);
  const resized = look(2, appearanceWith({ sizeBy: "weight" }));
  assert.ok(!(resized.weights === second.weights), "sizeBy changed, so the weights are new");
  assert.ok(resized.colours === second.colours, "and the colours, which did not, are not");

  const third = look(2);
  const regrouped = look(2, APPEARANCE, BY_DEGREE, GROUPS);
  assert.ok(!(regrouped.colours === third.colours), "the groups changed, so the colours are new");
  assert.ok(regrouped.weights === third.weights, "and the weights, which did not, are not");
});

test("a new filter replaces the mask and leaves the colours alone", () => {
  const base = look(null);
  const refiltered = look(null, APPEARANCE, BY_TEXT);
  assert.deepEqual([...maskOf(refiltered)], [0, 1, 1, 1, 1], "the text filter hides all but Alpha");
  assert.ok(refiltered.colours === base.colours, "the colours did not change and are not rebuilt");
  assert.ok(refiltered.weights === base.weights, "and neither did the weights");
});

test("a new graph replaces everything", () => {
  const base = look(2);
  const other = styleInputOf({
    meta: metaOf(NODES.slice(0, 3), ["a", "b", "c"], { source: Uint32Array.of(0), target: Uint32Array.of(1) }),
    appearance: APPEARANCE, filter: BY_DEGREE, groups: NO_GROUPS, analysis: null, reveal: 2,
  });
  assert.equal(other.colours.length, 3, "the new graph's three nodes are what is coloured");
  assert.ok(!(other.colours === base.colours), "so the colours cannot be the old graph's");
  assert.ok(!(other.weights === base.weights), "and the weights cannot be either");
});

test("the pipeline's reveal steps reuse the arrays too, at the view's edge", async () => {
  const made = desk(scriptedClient());
  const entry = await made.studio.start();
  assert.equal(entry.ok, true, entry.message);
  const sized = await made.studio.dispatch("appearance.size", { by: "degree" });
  assert.equal(sized.ok, true, sized.message);
  made.pipeline.reveal(1);
  const before = made.seen.styles.at(-1);
  made.pipeline.reveal(2);
  const after = made.seen.styles.at(-1);
  assert.ok(before !== undefined && after !== undefined, "both steps reached the view");
  assert.ok(after.colours === before.colours, "the view was handed the same colours array");
  assert.ok(after.weights === before.weights, "and the same weights array");
  assert.ok(!(after.hidden === before.hidden), "while the mask is rebuilt for the new rows");
});

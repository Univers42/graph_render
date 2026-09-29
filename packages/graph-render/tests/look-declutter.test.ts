/**
 * The greedy declutter of screen-space label boxes and the importance ranking that
 * feeds it.
 *   SciGraphs/core/visualization/text_overlay.py:302-326 (the declutter)
 *   SciGraphs/core/repro/executor.py:498-507 (rank, declutter, then the cut)
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { declutter, rankedLabels, selectLabels } from "../src/labels2d/declutter.ts";

test("two boxes far apart are both kept", () => {
  const kept = declutter([{ x: 0, y: 0, text: "aa" }, { x: 500, y: 500, text: "bb" }], 26);
  assert.deepEqual(kept, [0, 1]);
});

test("two overlapping boxes keep the first", () => {
  const kept = declutter([{ x: 10, y: 10, text: "Valjean" }, { x: 12, y: 10, text: "Javert" }], 26);
  assert.deepEqual(kept, [0]);
});

test("boxes that only touch are both kept on all four comparisons, not one (text_overlay.py:320-321)", () => {
  // half_w for a 4-character label at 26 px is 0.30*26*4 = 31.2, half_h 0.62*26.
  // The overlap test is a strict < on left/right and top/bottom, so a box that
  // only touches is not overlapping whichever of the two is the newcomer.
  const wide = "abcd";
  assert.deepEqual(declutter([{ x: 0, y: 0, text: wide }, { x: 62.4, y: 0, text: wide }], 26), [0, 1]);
  assert.deepEqual(declutter([{ x: 62.4, y: 0, text: wide }, { x: 0, y: 0, text: wide }], 26), [0, 1]);
  // 2 * half_h is the touching distance in y, and both orders have to pass.
  const dy = 2 * (0.62 * 26);
  assert.deepEqual(declutter([{ x: 0, y: 0, text: "a" }, { x: 0, y: dy, text: "a" }], 26), [0, 1]);
  assert.deepEqual(declutter([{ x: 0, y: dy, text: "a" }, { x: 0, y: 0, text: "a" }], 26), [0, 1]);
});

test("the width coefficient 0.30 is bounded from below as well as above (text_overlay.py:317)", () => {
  // half_w 31.2, so a 62.3 px gap is just inside the collision and a 62.4 px gap
  // just outside it. A 0.29 or 0.10 coefficient would keep both of these.
  assert.deepEqual(declutter([{ x: 0, y: 0, text: "abcd" }, { x: 62.3, y: 0, text: "abcd" }], 26), [0]);
});

test("the box is half_h = 0.62*font tall, so a vertical stack collides just inside it", () => {
  // half_h = 0.62*26 = 16.12, so two boxes touch at a 32.24 px gap: 32 collides,
  // 32.5 does not. A 0.63 coefficient would move that boundary to 32.76.
  const stacked = (dy: number) => declutter([{ x: 0, y: 0, text: "a" }, { x: 0, y: dy, text: "a" }], 26);
  assert.deepEqual(stacked(32), [0]);
  assert.deepEqual(stacked(32.5), [0, 1]);
  assert.deepEqual(stacked(40), [0, 1]);
});

test("half_h floors the font size at 1, so 0.62 and not 0.31 is the height (text_overlay.py:314)", () => {
  // half_w is 0.30*0.5 = 0.15 per character, so a 1 px gap still collides in x.
  // Without the floor half_h would be 0.31 and the two boxes would clear.
  assert.deepEqual(declutter([{ x: 0, y: 0, text: "a" }, { x: 0, y: 1, text: "a" }], 0.5), [0]);
});

test("a font size of zero leaves a zero-width box, so nothing collides and no NaN appears", () => {
  // half_w = 0.30*0*len = 0, while half_h = 0.62*max(0, 1) = 0.62 (text_overlay.py:314,317).
  assert.deepEqual(declutter([{ x: 0, y: 0, text: "aa" }, { x: 1, y: 0, text: "aa" }], 0), [0, 1]);
  assert.deepEqual(declutter([{ x: 0, y: 0, text: "aa" }, { x: 0, y: 0, text: "aa" }], 0), [0, 1]);
});

test("a box is compared with every accepted box, and a rejected one blocks nothing", () => {
  // Index 1 is far from index 0, index 2 overlaps index 0. Comparing with the
  // previous label only gives the same answer here, so the run below is the case
  // that tells them apart.
  const apart = [
    { x: 0, y: 0, text: "a" },
    { x: 500, y: 0, text: "a" },
    { x: 10, y: 0, text: "a" },
  ];
  assert.deepEqual(declutter(apart, 26), [0, 1]);
  // Three in a row: index 1 is rejected, so index 2 is compared with index 0 alone.
  const run = [
    { x: 0, y: 0, text: "a" },
    { x: 10, y: 0, text: "a" },
    { x: 20, y: 0, text: "a" },
  ];
  assert.deepEqual(declutter(run, 26), [0, 2]);
});

test("an empty label counts as one character wide", () => {
  const boxes = [{ x: 0, y: 0, text: "" }, { x: 0, y: 0, text: "" }];
  assert.deepEqual(declutter(boxes, 26), [0]);
  const apart = [{ x: 0, y: 0, text: "" }, { x: 30, y: 0, text: "" }];
  assert.deepEqual(declutter(apart, 26), [0, 1]);
});

test("the order given is the order of acceptance", () => {
  const boxes = [
    { x: 10, y: 10, text: "a" },
    { x: 12, y: 10, text: "b" },
    { x: 500, y: 10, text: "c" },
  ];
  assert.deepEqual(declutter(boxes, 26), [0, 2]);
  // Reversed, the far label is accepted first and index 1 survives instead of index 0.
  assert.deepEqual(declutter([...boxes].reverse(), 26), [0, 1]);
});

test("the width counts code points, as Python len() does, not UTF-16 units (text_overlay.py:317)", () => {
  // U+1D49C is one code point and two UTF-16 units, so half_w is 7.8 for it and
  // not 15.6, and two of them 20 px apart do not collide.
  const box = { x: 0, y: 0, text: "\u{1D49C}" };
  assert.deepEqual(declutter([box, { x: 20, y: 0, text: "\u{1D49C}" }], 26), [0, 1]);
});

test("no labels means nothing kept", () => {
  assert.deepEqual(declutter([], 26), []);
});

test("rankedLabels is score descending, index ascending on ties, the whole ranking", () => {
  // The sort key of executor.py:498-501; the ranking is not cut here, because
  // the source declutters the whole of it before it cuts (:503-507).
  const scores = Float64Array.from([0.2, 0.9, 0.2, 1, 0.9]);
  assert.deepEqual([...rankedLabels(scores)], [3, 1, 4, 0, 2]);
  assert.deepEqual([...rankedLabels(Uint32Array.from([3, 1, 2]))], [0, 2, 1]);
  assert.deepEqual([...rankedLabels(Uint32Array.from([7, 9, 7, 9, 7]))], [1, 3, 0, 2, 4]);
});

test("a missing score ranks last and leaves the finite order intact (executor.py:498-501)", () => {
  // The source key is (value is None, -(value or 0.0)), so a None sorts after
  // every present value and the rest stay descending among themselves.
  assert.deepEqual([...rankedLabels(Float64Array.from([1, Number.NaN, 3]))], [2, 0, 1]);
  assert.deepEqual([...rankedLabels(Float64Array.from([Number.NaN, 1, 3, 2]))], [2, 3, 1, 0]);
  assert.deepEqual(
    [...rankedLabels(Float64Array.from([5, Number.NaN, 1, Number.NaN, 9, 3, 7]))],
    [4, 6, 0, 5, 2, 1, 3],
  );
});

test("selectLabels ranks, then declutters, then cuts, as executor.py:498-507 does", () => {
  // The three highest scores are stacked on top of each other, so the declutter
  // drops two of them and the cut still has three places left to fill.
  const scores = Float64Array.from([6, 5, 4, 3, 2, 1]);
  const boxes = [
    { x: 0, y: 0, text: "a" },
    { x: 2, y: 0, text: "a" },
    { x: 4, y: 0, text: "a" },
    { x: 500, y: 0, text: "a" },
    { x: 1000, y: 0, text: "a" },
    { x: 1500, y: 0, text: "a" },
  ];
  assert.deepEqual(selectLabels(scores, boxes, 26, 3), [0, 3, 4]);
  assert.deepEqual(selectLabels(scores, boxes, 26, 18), [0, 3, 4, 5]);
  // Cutting to the limit before the declutter would leave the top label alone.
  assert.deepEqual(rankedLabels(scores), Uint32Array.from([0, 1, 2, 3, 4, 5]));
});

test("a max_count of 0 or below is no cut at all, the way executor.py:506-507 guards it", () => {
  const scores = Float64Array.from([1, 2]);
  const boxes = [{ x: 0, y: 0, text: "a" }, { x: 500, y: 0, text: "a" }];
  assert.deepEqual(selectLabels(scores, boxes, 26, 0), [1, 0]);
  assert.deepEqual(selectLabels(scores, boxes, 26, -1), [1, 0]);
  assert.deepEqual(selectLabels(scores, boxes, 26, 1), [1]);
});

/**
 * The tween's two timing marks, and the node budget past which the 2D painter snaps instead of
 * easing a layout switch.
 *
 * The marks are what `deploy/perf/transition.py` reads to time a switch from the click to the
 * settled frame, so a mark that fires twice for one tween would make the tween look twice as
 * long and one that never fires would hang the probe.
 *
 * The control is the last test in each block: `markTween` has to mark again for the *next* tween,
 * and `overBudget` has to say no to a scene under the budget, or the suite could pass with a
 * budget of zero and a mark that fires once ever.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { TWEEN_BUDGET, overBudget } from "../src/canvas2d/tween.ts";
import { MOVED_MARK, SETTLED_MARK, markTween } from "../src/transition.ts";

const marks = (name: string): number[] => performance.getEntriesByName(name).map((entry) => entry.startTime);

/** The user-timing buffer is process-wide and node runs every test in one, so each starts empty. */
function fresh(): object {
  performance.clearMarks(MOVED_MARK);
  performance.clearMarks(SETTLED_MARK);
  return {};
}

test("a tween marks that it moved once, however many frames it runs for", () => {
  const state = fresh();
  for (let frame = 0; frame < 36; frame += 1) markTween(state, 1000, false);
  assert.equal(marks(MOVED_MARK).length, 1, "thirty-six moving frames, one mark");
});

test("a tween marks that it settled once", () => {
  const state = fresh();
  markTween(state, 1000, false);
  markTween(state, 1000, true);
  markTween(state, 1000, true);
  assert.equal(marks(SETTLED_MARK).length, 1);
});

test("the control: the next tween marks both again", () => {
  const state = fresh();
  markTween(state, 1000, false);
  markTween(state, 1000, true);
  markTween(state, 2000, false);
  markTween(state, 2000, true);
  assert.equal(marks(MOVED_MARK).length, 2, "a second layout switch has to be measurable too");
  assert.equal(marks(SETTLED_MARK).length, 2);
});

test("a tween the 2D painter snaps has an end and no beginning", () => {
  const state = fresh();
  markTween(state, 1000, true);
  assert.equal(marks(SETTLED_MARK).length, 1, "a probe waiting for the end must not wait for a start");
  assert.equal(marks(MOVED_MARK).length, 0, "the nodes never eased anywhere");
  markTween(state, 1000, true);
  assert.equal(marks(SETTLED_MARK).length, 1, "and it still ends only once");
});

test("the control: a second snapped switch marks its end too", () => {
  const state = fresh();
  markTween(state, 1000, true);
  markTween(state, 2000, true);
  markTween(state, 3000, true);
  assert.equal(marks(SETTLED_MARK).length, 3, "every snap over the budget is a measurable switch");
});

test("the control: a snap then a real tween marks both ends of the tween", () => {
  const state = fresh();
  markTween(state, 1000, true);
  markTween(state, 2000, false);
  markTween(state, 2000, true);
  assert.equal(marks(MOVED_MARK).length, 1);
  assert.equal(marks(SETTLED_MARK).length, 2, "the snap's end and the tween's end");
});

test("a tween of a scene under the budget is eased by the 2D painter", () => {
  assert.equal(overBudget({ bulk: 0 }, TWEEN_BUDGET), false, "the budget itself still tweens");
  assert.equal(overBudget({ bulk: 0 }, TWEEN_BUDGET - 1), false);
});

test("a tween of a scene over the budget snaps, unless the GPU layer has the frame", () => {
  assert.equal(overBudget({ bulk: 0 }, TWEEN_BUDGET + 1), true, "the 2D painter cannot ease this one");
  assert.equal(overBudget({ bulk: 2 }, TWEEN_BUDGET + 1), false, "the layer mixes it for one float a frame");
  assert.equal(overBudget({ bulk: 2 }, 1_000_000), false, "so even a million nodes still tween on the GPU");
});
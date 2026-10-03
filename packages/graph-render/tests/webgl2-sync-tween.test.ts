/**
 * Which frames make the node columns reach the driver, and above all which ones do not: the whole
 * of what a layout switch costs on the GPU is that a tween's four columns go up once and the
 * eased uniform moves them after that.
 *
 * WHY the policy is a function and not a line inside `syncNodes`: node has no WebGL2 context, and
 * a missing upload is invisible to every other test in the package — the layer would still draw a
 * moving tween, just from the columns it happened to hold, and a picture cannot tell. This is the
 * decision on its own, so the suite can say "a tween uploads once" rather than "still looks like
 * it moves".
 *
 * The controls are the last two tests: a first tween has to ask for an upload, and a tween that
 * ends has to put the plain gate back, or the suite could pass with a layer that uploads once and
 * then never draws anything else again.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Pace, Tween } from "../src/webgl2/layer.ts";
import { type PositionKey, positionsDue } from "../src/webgl2/sync.ts";

const column = (at: number): Float32Array => Float32Array.from({ length: 4 }, (_, node) => at + node);

const pace = (placed: number, tween: Tween | null): Pace => ({ placed, budget: 1, tween });
const tween = (fromX: Float32Array, toX: Float32Array, eased = 0.5): Tween =>
  ({ fromX, fromY: column(0), toX, toY: column(100), eased });

/** A layer that has taken nothing yet. */
const empty: PositionKey = { x: null, placed: -1, fromX: null, toX: null };

test("outside a tween, a new column has to be uploaded", () => {
  assert.equal(positionsDue(empty, column(0), pace(0, null)), true);
});

test("outside a tween, the same column and the same counter are already up", () => {
  const x = column(0);
  assert.equal(positionsDue({ ...empty, x, placed: 4 }, x, pace(4, null)), false);
});

test("outside a tween, the counter moving means the columns were written in place", () => {
  const x = column(0);
  assert.equal(positionsDue({ ...empty, x, placed: 4 }, x, pace(5, null)), true);
});

test("a tween's next frame uploads nothing, however far it has eased", () => {
  const from = column(0);
  const to = column(100);
  const first = positionsDue(empty, to, pace(0, tween(from, to)));
  assert.equal(first, true, "the four columns have to reach the driver once");
  assert.equal(positionsDue({ x: to, placed: 1, fromX: from, toX: to }, to, pace(1, tween(from, to, 0.9))), false);
  assert.equal(positionsDue({ x: to, placed: 30, fromX: from, toX: to }, to, pace(30, tween(from, to, 0.999))), false);
});

test("a tween whose columns changed does upload again", () => {
  const to = column(100);
  const was = { x: to, placed: 1, fromX: column(0), toX: to };
  assert.equal(positionsDue(was, to, pace(2, tween(column(50), to))), true, "a second switch has new columns");
});

test("the control: a tween that ends puts the plain gate back", () => {
  const to = column(100);
  const during = { x: to, placed: 9, fromX: column(0), toX: to };
  assert.equal(positionsDue(during, column(0), pace(9, null)), true, "the eased pose is not the target");
  assert.equal(positionsDue({ ...during, x: to }, to, pace(9, null)), false, "and the target is already up");
});

test("the control: a settled frame after a tween needs no upload at all", () => {
  const to = column(100);
  const during = { x: to, placed: 30, fromX: column(0), toX: to };
  assert.equal(positionsDue(during, to, pace(30, null)), false,
    "the tween left the target columns in `x` and the counter it will be compared against");
});
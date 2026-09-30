/** When the label layout must run again: a camera change, and nothing that only redraws. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Keyed, layoutChanged } from "../src/canvas2d/layout-key.ts";

/** The view writes these fields in place; the test does too. */
type Mutable<T> = { -readonly [K in keyof T]: T[K] };

function keyed(): Mutable<Keyed> {
  return {
    scene: {}, theme: {}, policy: {}, x: {}, camera: { x: 0, y: 0, scale: 1 },
    viewport: { width: 800, height: 600 }, dpr: 1, sprites: { baked: () => 0 },
    layoutKey: null, layoutDirty: false,
  };
}

test("the first frame lays out, an identical second one does not", () => {
  const state = keyed();
  assert.equal(layoutChanged(state, -1, false), true);
  assert.equal(layoutChanged(state, -1, false), false);
  assert.equal(layoutChanged(state, -1, false), false);
});

test("a zoom or a pan lays out again, once", () => {
  const state = keyed();
  layoutChanged(state, -1, false);
  state.camera = { x: 0, y: 0, scale: 2 };
  assert.equal(layoutChanged(state, -1, false), true);
  assert.equal(layoutChanged(state, -1, false), false);
  state.camera = { x: 5, y: 0, scale: 2 };
  assert.equal(layoutChanged(state, -1, false), true);
});

test("a redraw with the same hover, and a moved node marked dirty", () => {
  const state = keyed();
  layoutChanged(state, 4, false);
  assert.equal(layoutChanged(state, 4, false), false);
  assert.equal(layoutChanged(state, 5, false), true);
  state.layoutDirty = true;
  assert.equal(layoutChanged(state, 5, false), true);
  assert.equal(state.layoutDirty, false);
});

test("nodes in flight, a new bake, a new scene or theme each lay out again", () => {
  const state = keyed();
  layoutChanged(state, -1, false);
  assert.equal(layoutChanged(state, -1, true), true);
  assert.equal(layoutChanged(state, -1, true), true);
  let baked = 0;
  state.sprites = { baked: () => baked };
  layoutChanged(state, -1, false);
  baked = 1;
  assert.equal(layoutChanged(state, -1, false), true);
  state.scene = {};
  assert.equal(layoutChanged(state, -1, false), true);
  state.theme = {};
  assert.equal(layoutChanged(state, -1, false), true);
});

import assert from "node:assert/strict";
import { test } from "node:test";

import { createSpriteCache } from "../src/canvas2d/sprites.ts";
import type { SpriteSurface, TextSurface2D } from "../src/canvas2d/surface.ts";
import { DARK_THEME, LIGHT_THEME } from "../src/theme.ts";

interface Bench {
  surfaces: number;
  bakes: number;
  factory: () => SpriteSurface<string>;
}

function bench(): Bench {
  const state: Bench = {
    surfaces: 0,
    bakes: 0,
    factory: () => {
      state.surfaces += 1;
      const ctx: TextSurface2D = {
        font: "", fillStyle: "", strokeStyle: "", lineWidth: 1, lineJoin: "round", textBaseline: "middle",
        setTransform: () => undefined,
        measureText: (text) => ({ width: text.length * 7 }),
        strokeText: () => undefined,
        fillText: () => {
          state.bakes += 1;
        },
      };
      return { image: `surface ${state.surfaces}`, ctx, resize: () => undefined };
    },
  };
  return state;
}

test("a label is baked once and measured from its text", () => {
  const made = bench();
  const cache = createSpriteCache(made.factory, DARK_THEME);
  assert.equal(cache.widthOf("hello"), 0);
  const first = cache.get("hello");
  assert.equal(first?.width, 5 * 7 + 8);
  assert.equal(cache.get("hello"), first);
  assert.equal(made.bakes, 1);
  assert.equal(cache.widthOf("hello"), 43);
});

test("a frame bakes at most its allowance and says it ran out", () => {
  const made = bench();
  const cache = createSpriteCache(made.factory, DARK_THEME);
  cache.beginFrame();
  const got = Array.from({ length: 40 }, (_, i) => cache.get(`label ${i}`)).filter((sprite) => sprite !== null);
  assert.equal(got.length, 32);
  assert.equal(cache.starved(), true);
  cache.beginFrame();
  assert.equal(cache.starved(), false);
  assert.notEqual(cache.get("label 39"), null);
});

test("past the capacity the least recently drawn surface is reused", () => {
  const made = bench();
  const cache = createSpriteCache(made.factory, DARK_THEME);
  for (let i = 0; i < 512; i += 1) {
    if (i % 32 === 0) cache.beginFrame();
    cache.get(`label ${i}`);
  }
  cache.beginFrame();
  cache.get("label 0");
  cache.get("one more");
  assert.equal(made.surfaces, 512);
  assert.equal(cache.widthOf("label 1"), 0);
  assert.notEqual(cache.widthOf("label 0"), 0);
});

test("a new theme or pixel ratio re-bakes on the surfaces it already has", () => {
  const made = bench();
  const cache = createSpriteCache(made.factory, DARK_THEME);
  cache.get("hello");
  cache.reset(DARK_THEME, 1);
  assert.notEqual(cache.widthOf("hello"), 0);
  cache.reset(LIGHT_THEME, 2);
  assert.equal(cache.widthOf("hello"), 0);
  cache.get("hello");
  assert.deepEqual([made.surfaces, made.bakes], [1, 2]);
});

test("a long label is cut, and a host with no canvas gets no sprite", () => {
  const made = bench();
  const cache = createSpriteCache(made.factory, DARK_THEME);
  assert.equal(cache.get("x".repeat(200))?.width, 48 * 7 + 8);
  assert.equal(createSpriteCache(() => null, DARK_THEME).get("hello"), null);
});

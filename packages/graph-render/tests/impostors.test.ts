/**
 * The impostor cache: one lit sphere per (base colour, size), baked once with shadeSphere
 * and blitted after. Pins the bake, the key, the per-frame allowance and the refusal when
 * the host has nowhere to put pixels.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { createImpostorCache } from "../src/canvas2d/impostors.ts";
import type { SpriteSurface, TextSurface2D } from "../src/canvas2d/surface.ts";
import { shadeSphere } from "../src/sprite/impostor.ts";
import type { Rgb } from "../src/colour/srgb.ts";

const RED: Rgb = [0.8, 0.2, 0.1];
const BLUE: Rgb = [0.1, 0.2, 0.8];

interface Pixels {
  size: number;
  data: Uint8ClampedArray;
}

interface Bench {
  readonly factory: () => SpriteSurface<Pixels>;
  bakes: number;
  resizes: number;
}

function bench(withPixels: boolean): Bench {
  const state: Bench = {
    bakes: 0,
    resizes: 0,
    factory: () => {
      const pixels: Pixels = { size: 0, data: new Uint8ClampedArray(new ArrayBuffer(0)) };
      const ctx: TextSurface2D = {
        font: "", fillStyle: "", strokeStyle: "", lineWidth: 1, lineJoin: "round", textBaseline: "middle",
        setTransform: () => undefined,
        measureText: (text) => ({ width: text.length * 7 }),
        fillRect: () => undefined,
        strokeText: () => undefined,
        fillText: () => undefined,
      };
      return {
        image: pixels,
        ctx,
        ...(withPixels
          ? { pixels: { putPixels: (data: Uint8ClampedArray) => { state.bakes += 1; pixels.data = data; } } }
          : {}),
        resize: (width) => {
          state.resizes += 1;
          pixels.size = width;
        },
      };
    },
  };
  return state;
}

test("a sphere is baked once at the size asked for, from shadeSphere", () => {
  const made = bench(true);
  const cache = createImpostorCache<Pixels>(made.factory);
  const sprite = cache.get(RED, 16);
  assert.ok(sprite, "the cache bakes a sphere for an integral size");
  assert.equal(sprite.width, 16);
  assert.equal(sprite.height, 16);
  assert.deepEqual(sprite.image.data, shadeSphere(RED, 16));
  assert.equal(made.bakes, 1);
  assert.equal(made.resizes, 1);
});

test("the same colour and size is one bake, a different colour or size is another", () => {
  const made = bench(true);
  const cache = createImpostorCache<Pixels>(made.factory);
  const first = cache.get(RED, 16);
  assert.equal(cache.get(RED, 16), first);
  assert.equal(made.bakes, 1);
  cache.get(BLUE, 16);
  cache.get(RED, 24);
  assert.equal(made.bakes, 3);
  assert.equal(cache.get(RED, 24)?.width, 24);
});

test("a host with no pixels bakes nothing and draws no sphere", () => {
  const made = bench(false);
  const cache = createImpostorCache<Pixels>(made.factory);
  assert.equal(cache.get(RED, 16), null);
  assert.equal(made.bakes, 0);
  assert.equal(cache.starved(), false);
});

test("a size that is not a positive integer is refused, never thrown mid-frame", () => {
  const made = bench(true);
  const cache = createImpostorCache<Pixels>(made.factory);
  assert.equal(cache.get(RED, 0), null);
  assert.equal(cache.get(RED, 12.5), null);
  assert.equal(cache.get(RED, -4), null);
  assert.equal(made.bakes, 0);
});

test("a frame bakes at most its allowance and then says it is starved", () => {
  const made = bench(true);
  const cache = createImpostorCache<Pixels>(made.factory);
  cache.beginFrame();
  const got = Array.from({ length: 40 }, (_, i) => cache.get(i % 2 === 0 ? RED : BLUE, 8 + i))
    .filter((sprite) => sprite !== null);
  assert.equal(got.length, 32);
  assert.equal(cache.starved(), true);
  cache.beginFrame();
  assert.equal(cache.starved(), false);
  // The 32 already baked are cache hits; the next size is a fresh bake.
  assert.equal(cache.get(RED, 48)?.width, 48);
  assert.equal(made.bakes, 33);
});

test("a device pixel ratio change drops the baked spheres", () => {
  const made = bench(true);
  const cache = createImpostorCache<Pixels>(made.factory);
  cache.get(RED, 16);
  cache.reset(1);
  assert.equal(cache.get(RED, 16)?.width, 16);
  assert.equal(made.bakes, 1);
  cache.reset(2);
  cache.get(RED, 16);
  assert.equal(made.bakes, 2);
});

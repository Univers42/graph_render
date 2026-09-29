/**
 * shadeSphere: the sphere-impostor sprite, one lit sphere per colour, cached by the
 * painter and drawn scaled.
 *   SciGraphs/ui/gpu_render/glsl/sphere_frag.glsl (normal, z, rim term)
 *   SciGraphs/ui/gpu_render/draw.py:12 (headlight), 76-83 (ambient, no fill)
 *   SciGraphs/ui/gpu_render/properties.py:1106,1111 (ambient 0.25, rim 0.15)
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { shadeSphere } from "../src/sprite/impostor.ts";
import { srgbEncode } from "../src/colour/srgb.ts";

/** The length of the normalised key direction, (0.3, 0.3, 1). */
const KEY_LENGTH = Math.hypot(0.3, 0.3, 1);

function item(values: Uint8ClampedArray, index: number): number {
  const value = values[index];
  assert.ok(value !== undefined, `index ${index} out of range`);
  return value;
}

function pixel(tex: Uint8ClampedArray, size: number, px: number, py: number): readonly number[] {
  const base = (py * size + px) * 4;
  return [item(tex, base), item(tex, base + 1), item(tex, base + 2), item(tex, base + 3)];
}

function byteOf(c: number): number {
  return Math.round(255 * srgbEncode(Math.min(1, Math.max(0, c))));
}

/** The source's own formula for one pixel, written out again (sphere_frag.glsl:5-15). */
function shadeAt(base: readonly [number, number, number], size: number, px: number, py: number): readonly number[] {
  const x = ((px + 0.5) / size) * 2 - 1;
  const y = 1 - ((py + 0.5) / size) * 2;
  const r2 = x * x + y * y;
  if (r2 > 1) return [0, 0, 0, 0];
  const z = Math.sqrt(1 - r2);
  const key = Math.max((x * 0.3 + y * 0.3 + z) / KEY_LENGTH, 0);
  const lit = 0.25 + 0.9 * key;
  const rim = (1 - z) ** 3 * 0.15;
  return [0, 1, 2].map((c) => byteOf((base[c] ?? 0) * lit + rim)).concat(255);
}

test("the sprite is size*size RGBA", () => {
  assert.equal(shadeSphere([0.5, 0.5, 0.5], 8).length, 8 * 8 * 4);
  assert.equal(shadeSphere([0.5, 0.5, 0.5], 1).length, 4);
});

test("a size that is not a positive integer is refused, so the sprite is size*size RGBA", () => {
  assert.throws(() => shadeSphere([0.5, 0.5, 0.5], 2.5), RangeError);
  assert.throws(() => shadeSphere([0.5, 0.5, 0.5], 0), RangeError);
  assert.throws(() => shadeSphere([0.5, 0.5, 0.5], -1), RangeError);
});

test("the silhouette is exactly r2 > 1, so the pixel outside is fully transparent", () => {
  const size = 9;
  const base: readonly [number, number, number] = [0.6, 0.6, 0.7];
  const tex = shadeSphere(base, size);
  // p = ((0+0.5)/9*2-1, 1-(0+0.5)/9*2) = (-0.888.., 0.888..), r2 = 1.580 > 1.
  assert.deepEqual(pixel(tex, size, 0, 0), [0, 0, 0, 0]);
  // r2 = 1.234 > 1, so this one is outside too even though it is not a corner.
  assert.deepEqual(pixel(tex, size, 0, 1), [0, 0, 0, 0]);
  // r2 = 0.790 < 1 at the mid-edge, and r2 = 0.9877 < 1 one pixel further in.
  assert.equal(pixel(tex, size, 4, 0)[3], 255);
  assert.equal(pixel(tex, size, 2, 0)[3], 255);
});

test("the whole sprite is the shader's formula, inside the silhouette and out", () => {
  const size = 9;
  const base: readonly [number, number, number] = [0.4, 0.6, 0.8];
  const tex = shadeSphere(base, size);
  for (let py = 0; py < size; py += 1) {
    for (let px = 0; px < size; px += 1) {
      assert.deepEqual(pixel(tex, size, px, py), shadeAt(base, size, px, py), `pixel ${px},${py}`);
    }
  }
});

test("the silhouette threshold is 1 from both sides (sphere_frag.glsl:5)", () => {
  // The last pixel inside the edge and the first one outside bracket r2 = 1 from
  // both directions: at size 64 the closest inside pixel is r2 = 0.9985 and at
  // size 100 the closest outside one is r2 = 1.0018. So a threshold moved to
  // either side of 1, or rounded down to 0.99, changes one of these.
  const r2At = (size: number, px: number, py: number) => {
    const x = ((px + 0.5) / size) * 2 - 1;
    const y = 1 - ((py + 0.5) / size) * 2;
    return x * x + y * y;
  };
  const inside: readonly [number, number, number] = [64, 26, 0];
  assert.ok(r2At(...inside) > 0.99 && r2At(...inside) <= 1, "inside fixture");
  assert.equal(pixel(shadeSphere([0.5, 0.5, 0.5], inside[0]), ...inside)[3], 255);
  const outside: readonly [number, number, number] = [100, 31, 3];
  assert.ok(r2At(...outside) > 1 && r2At(...outside) < 1.01, "outside fixture");
  assert.equal(pixel(shadeSphere([0.5, 0.5, 0.5], outside[0]), ...outside)[3], 0);
});

test("the centre pixel of an odd size is the formula evaluated by hand", () => {
  const size = 9;
  const base: readonly [number, number, number] = [0.4, 0.6, 0.8];
  const tex = shadeSphere(base, size);
  // p = 0 at the centre, so r2 = 0, z = 1, n = (0,0,1) and n.key = key.z.
  const keyZ = 1 / KEY_LENGTH;
  const lit = 0.25 + 0.9 * Math.max(keyZ, 0);
  const rim = (1 - 1) ** 3 * 0.15;
  assert.deepEqual(pixel(tex, size, 4, 4), [
    byteOf(base[0] * lit + rim), byteOf(base[1] * lit + rim), byteOf(base[2] * lit + rim), 255,
  ]);
});

test("the top-right quadrant is brighter than the bottom-left", () => {
  const size = 16;
  const tex = shadeSphere([0.8, 0.8, 0.8], size);
  const topRight = pixel(tex, size, 12, 3);
  const bottomLeft = pixel(tex, size, 3, 12);
  assert.ok((topRight[0] ?? 0) > (bottomLeft[0] ?? 0), `${topRight[0]} vs ${bottomLeft[0]}`);
  assert.equal(topRight[3], 255);
  assert.equal(bottomLeft[3], 255);
});

test("a near-rim pixel carries the (1-z)^3*0.15 highlight, so the coefficient is pinned", () => {
  // size 9, pixel (2, 0): p = (-0.444.., 0.888..), z = 0.1111, (1-z)^3 = 0.7023.
  const size = 9;
  const x = ((2 + 0.5) / size) * 2 - 1;
  const y = 1 - ((0 + 0.5) / size) * 2;
  const z = Math.sqrt(1 - (x * x + y * y));
  const keyDot = Math.max((x * 0.3 + y * 0.3 + z) / KEY_LENGTH, 0);
  const lit = 0.8 * (0.25 + 0.9 * keyDot);
  const expected = byteOf(Math.min(1, lit + (1 - z) ** 3 * 0.15));
  assert.equal(pixel(shadeSphere([0.8, 0.8, 0.8], size), size, 2, 0)[0], expected);
  // A larger rim coefficient would raise that byte, so the two differ.
  assert.notEqual(byteOf(Math.min(1, lit + (1 - z) ** 3 * 0.16)), expected);
});

test("a pixel on the unlit crescent has its key dot clamped to zero (sphere_frag.glsl:11)", () => {
  // size 9, pixel (1, 7): p = (-0.666.., -0.666..), z = 0.3333 and
  // n.key = (-0.666-0.666+0.333)/1.0877 = -0.0614, so max(., 0) makes the pixel
  // ambient plus rim only. A raw dot would subtract light and darken it.
  const size = 9;
  const base: readonly [number, number, number] = [0.8, 0.8, 0.8];
  const px = 1;
  const py = 7;
  const x = ((px + 0.5) / size) * 2 - 1;
  const y = 1 - ((py + 0.5) / size) * 2;
  const z = Math.sqrt(1 - (x * x + y * y));
  assert.ok((x * 0.3 + y * 0.3 + z) / KEY_LENGTH < 0, "this pixel should be on the dark side");
  const lit = 0.8 * 0.25;
  const expected = byteOf(Math.min(1, lit + (1 - z) ** 3 * 0.15));
  assert.equal(pixel(shadeSphere(base, size), size, px, py)[0], expected);
  // Without the clamp the channel loses 0.9 * 0.8 * 0.0614 and reads darker.
  assert.notEqual(byteOf(Math.min(1, 0.8 * (0.25 + 0.9 * -0.0614) + (1 - z) ** 3 * 0.15)), expected);
});

test("a lit channel clamps at 1 rather than wrapping (sphere_frag.glsl:15)", () => {
  // base (1,1,1) at the centre gives lit = 0.25 + 0.9*0.9206 = 1.0785, so every
  // channel is past 1 before the encode. A wrapping clamp reads far lower.
  const tex = shadeSphere([1, 1, 1], 9);
  assert.deepEqual(pixel(tex, 9, 4, 4), [255, 255, 255, 255]);
});

test("a size of one puts the single pixel at the centre", () => {
  // At size 1 the only pixel has p = 0, so z = 1, n.key = key.z and the rim is 0.
  const keyZ = 1 / KEY_LENGTH;
  const expected = byteOf(0.5 * (0.25 + 0.9 * keyZ));
  assert.deepEqual([...shadeSphere([0.5, 0.5, 0.5], 1)], [expected, expected, expected, 255]);
});

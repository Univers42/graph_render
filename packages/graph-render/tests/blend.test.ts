// The blend primitive: the linear mix, the stops one edge carries, and the decode a palette
// entry comes back through.
import assert from "node:assert/strict";
import { test } from "node:test";

import { GRADIENT_STOPS, edgeStops, meanLinear, mixLinear } from "../src/colour/blend.ts";
import { byteOf, cssOf, linearOf, srgbDecode, srgbEncode } from "../src/colour/srgb.ts";

const BLACK: readonly [number, number, number] = [0, 0, 0];
const WHITE: readonly [number, number, number] = [1, 1, 1];

function bytes(css: string): readonly number[] {
  const found = /^rgb\((\d+), (\d+), (\d+)\)$/.exec(css);
  assert.ok(found !== null, `${css} is not an rgb() colour`);
  return [Number(found[1]), Number(found[2]), Number(found[3])];
}

test("K is a named constant of at least three stops, evenly spread over the edge", () => {
  const offsets = edgeStops(BLACK, WHITE).map((stop) => stop.offset);
  assert.equal(offsets.length, GRADIENT_STOPS);
  assert.deepEqual(offsets, [0, 0.25, 0.5, 0.75, 1]);
});

test("the ends of the ramp are the two colours exactly, encoded once", () => {
  const a: readonly [number, number, number] = [0.2, 0.4, 0.6];
  const b: readonly [number, number, number] = [0.8, 0.15, 0.05];
  const stops = edgeStops(a, b);
  assert.equal(stops[0]?.colour, cssOf(a));
  assert.equal(stops[GRADIENT_STOPS - 1]?.colour, cssOf(b));
});

test("the middle of the ramp is the linear mean, not the mean of the encoded ends", () => {
  const stops = edgeStops(BLACK, WHITE);
  const middle = bytes(stops[2]?.colour ?? "");
  // 0.5 linear encodes to 187.52 bytes; the mean of the two encoded ends would be 128.
  assert.deepEqual(middle, [188, 188, 188]);
  assert.deepEqual(bytes(stops[0]?.colour ?? ""), [0, 0, 0]);
  assert.deepEqual(bytes(stops[GRADIENT_STOPS - 1]?.colour ?? ""), [255, 255, 255]);
});

test("the mix at the stops is the hand-computed linear value, within one byte", () => {
  const a: readonly [number, number, number] = [0.1, 0.5, 0.25];
  const b: readonly [number, number, number] = [0.9, 0.25, 0.75];
  const stops = edgeStops(a, b);
  for (const stop of stops) {
    for (let channel = 0; channel < 3; channel += 1) {
      const want = byteOf((a[channel] ?? 0) + ((b[channel] ?? 0) - (a[channel] ?? 0)) * stop.offset);
      const got = bytes(stop.colour)[channel] ?? -1;
      assert.ok(Math.abs(got - want) <= 1, `at ${stop.offset} channel ${channel}: ${got} against ${want}`);
    }
  }
});

test("mixLinear clamps t outside 0..1 and meanLinear is the mix at one half", () => {
  assert.deepEqual(mixLinear(BLACK, WHITE, -1), BLACK);
  assert.deepEqual(mixLinear(BLACK, WHITE, 2), WHITE);
  assert.deepEqual(meanLinear(BLACK, WHITE), [0.5, 0.5, 0.5]);
  assert.deepEqual(meanLinear(BLACK, WHITE), mixLinear(BLACK, WHITE, 0.5));
});

test("the decode is the exact inverse of the encode, and a byte round trips", () => {
  for (let byte = 0; byte <= 255; byte += 1) {
    const back = srgbDecode(byte / 255);
    assert.equal(byteOf(back), byte, `byte ${byte} came back as ${byteOf(back)}`);
  }
  for (const value of [0, 0.0031308, 0.04045, 0.5, 1]) {
    assert.ok(Math.abs(srgbDecode(srgbEncode(value)) - value) < 1e-6, `${value} did not survive`);
  }
});

test("linearOf reads the three spellings a palette entry comes in and refuses the rest", () => {
  assert.deepEqual(linearOf("#ff0000"), [1, 0, 0]);
  assert.deepEqual(linearOf("#f00"), [1, 0, 0]);
  assert.deepEqual(linearOf("rgb(255, 0, 0)"), [1, 0, 0]);
  assert.deepEqual(linearOf("  #0000FF  "), [0, 0, 1]);
  assert.equal(linearOf("sepia"), null);
  assert.equal(linearOf("rgba(255, 0, 0, 0.5)"), null);
  assert.equal(linearOf("#ff00"), null);
});

test("a palette entry read back out is the very colour it was written from", () => {
  for (const linear of [[0.2, 0.4, 0.6], [1, 1, 1], [0.03, 0.07, 0.11]] as const) {
    const css = cssOf(linear);
    const read = linearOf(css);
    assert.ok(read !== null);
    assert.equal(cssOf(read), css);
  }
});
/** srgbEncode, cssOf, byteOf: the one encode step every drawn colour goes through. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { byteOf, cssOf, srgbEncode } from "../src/colour/srgb.ts";

/** The reference formula, written out again so the test does not just call the code. */
function reference(c: number): number {
  const x = Math.min(1, Math.max(0, c));
  return x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055;
}

test("the ends of the ramp are exact", () => {
  assert.equal(srgbEncode(0), 0);
  // 1.055 - 0.055 is not 1 in binary, so the formula lands one ulp under 1; the
  // byte it produces is still 255. Reported as a deviation from the brief.
  assert.ok(Math.abs(srgbEncode(1) - 1) < 1e-15);
});

test("below the threshold the encode is the 12.92x branch, at 0.002", () => {
  assert.equal(srgbEncode(0.002), 12.92 * 0.002);
});

test("above the threshold the encode is the 1.055c^(1/2.4)-0.055 branch, at 0.5", () => {
  assert.ok(Math.abs(srgbEncode(0.5) - (1.055 * 0.5 ** (1 / 2.4) - 0.055)) < 1e-15);
});

test("the 0.0031308 knee is inside the linear branch and 0.004 is not (target.py:23)", () => {
  // The comparison is <=, so the knee itself takes 12.92x. Moving the knee to
  // 0.01 would send 0.004 down the linear branch and change this byte.
  assert.equal(srgbEncode(0.0031308), 12.92 * 0.0031308);
  assert.equal(srgbEncode(0.004), 1.055 * 0.004 ** (1 / 2.4) - 0.055);
  assert.equal(srgbEncode(0.004), 0.05070871397734796);
});

test("the two branches meet at 0.0031308", () => {
  const threshold = 0.0031308;
  const below = 12.92 * threshold;
  const above = 1.055 * threshold ** (1 / 2.4) - 0.055;
  assert.ok(Math.abs(below - above) < 1e-6, `gap ${Math.abs(below - above)}`);
});

test("a value outside 0..1 is clamped, never extrapolated", () => {
  assert.equal(srgbEncode(-1), 0);
  assert.ok(Math.abs(srgbEncode(4) - 1) < 1e-15);
});

test("cssOf of the gallery background matches byteOf per channel", () => {
  const background: readonly [number, number, number] = [0.035, 0.035, 0.045];
  const expected = background.map((c) => Math.round(255 * reference(c))).join(", ");
  assert.equal(cssOf(background), `rgb(${expected})`);
});

test("byteOf is the rounded encode of one channel", () => {
  assert.equal(byteOf(0.035), Math.round(255 * reference(0.035)));
  assert.equal(byteOf(1), 255);
});

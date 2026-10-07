// The kernel refresh's host half: `sampleKernel` is `kernel::sample`, held to the fixture.
//
// The fixture's spectrum is graph-core's kernel for the fixture's own frame, sampled and then
// forward transformed (`kernel.rs:42-63`). A direct 2D DFT of `sampleKernel`'s samples, at a
// spread of frequencies, must give the fixture's spectrum at the same frequencies: that pins
// the sampling (offsets, wrap, law, scale) and the transposed layout the device's `gain`
// holds, `gain[kx · side + ky]`. The device's own forward passes are the charge probe's.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-kernel.test.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

/** The committed 1k fixture, as the bytes the page would fetch. */
function committed(state) {
  const bytes = readFileSync(new URL(`../../../fixtures/gpu/mesh-1k-${state}.gmfx`, import.meta.url));
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
}

/** The forward DFT of the interleaved samples `g` at `(kx, ky)`, in f64. */
function dft(g, side, kx, ky) {
  let re = 0;
  let im = 0;
  for (let y = 0; y < side; y += 1) {
    for (let x = 0; x < side; x += 1) {
      const at = (y * side + x) * 2;
      if (g[at] === 0 && g[at + 1] === 0) {
        continue;
      }
      const angle = (-2 * Math.PI * (((kx * x) % side) + ((ky * y) % side))) / side;
      const [c, s] = [Math.cos(angle), Math.sin(angle)];
      re += g[at] * c - g[at + 1] * s;
      im += g[at] * s + g[at + 1] * c;
    }
  }
  return [re, im];
}

test("the_kernel_sample_transforms_to_the_fixture_spectrum", async () => {
  const { loadFixture } = await import("../src/gpu/fixture.ts");
  const { sampleKernel } = await import("../src/gpu/charge-kernel.ts");
  for (const state of ["start", "settled"]) {
    const fixture = loadFixture(committed(state));
    const { side } = fixture;
    const g = sampleKernel(side, fixture);
    let peak = 0;
    for (let k = 0; k < side * side; k += 1) {
      peak = Math.max(peak, Math.abs(fixture.spectrumRe[k]), Math.abs(fixture.spectrumIm[k]));
    }
    for (const [kx, ky] of [[0, 0], [1, 0], [0, 1], [3, 5], [side / 2, 7], [side - 1, side - 2], [17, side / 4]]) {
      const [re, im] = dft(g, side, kx, ky);
      const at = kx * side + ky;
      assert.ok(Math.abs(re - fixture.spectrumRe[at]) <= 1e-12 * peak, `${state} (${kx}, ${ky}) re ${re} vs ${fixture.spectrumRe[at]}`);
      assert.ok(Math.abs(im - fixture.spectrumIm[at]) <= 1e-12 * peak, `${state} (${kx}, ${ky}) im ${im} vs ${fixture.spectrumIm[at]}`);
    }
  }
});

test("a_kernel_on_another_rung_is_another_spectrum", async () => {
  // The negative half: the next rung's samples sit 2^(1/4) further apart, so the same DFT
  // misses the fixture's spectrum by far more than the tolerance above.
  const { loadFixture } = await import("../src/gpu/fixture.ts");
  const { sampleKernel } = await import("../src/gpu/charge-kernel.ts");
  const fixture = loadFixture(committed("start"));
  const g = sampleKernel(fixture.side, { h: fixture.h * 2 ** 0.25, reach: fixture.reach });
  const [re] = dft(g, fixture.side, 1, 0);
  assert.ok(Math.abs(re - fixture.spectrumRe[fixture.side]) > 1e-3 * Math.abs(fixture.spectrumRe[fixture.side]));
});

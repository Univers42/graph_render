/**
 * normalise() mirrors SciGraphs' normalize_values: percentile clip, vmin/vmax
 * override, transform, clip to 0..1, then gamma.
 *   SciGraphs/core/scigraphs_core/coloring/colormaps.py:411-555
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { NORM_MODES, normalise } from "../src/colour/normalise.ts";

/** Indexed reads go through this so a wrong index fails the test instead of the type. */
function at(values: Float64Array, index: number): number {
  const value = values[index];
  assert.ok(value !== undefined, `index ${index} out of range`);
  return value;
}

/** The 77-value fixture from the studio look spec: 43 ties at zero, 34 distinct positives. */
function ties(): number[] {
  const values: number[] = [];
  for (let i = 0; i < 43; i += 1) values.push(0);
  for (let i = 1; i <= 34; i += 1) values.push(i);
  return values;
}

test("the four modes are the ones SciGraphs offers (colormaps.py:338-366)", () => {
  assert.deepEqual([...NORM_MODES], ["LINEAR", "LOG", "RANK", "QUANTILE"]);
});

test(
  "RANK on 43 zeros and 34 distinct positives gives every zero exactly 21/76 (colormaps.py:462-469,527-530)",
  () => {
    const out = normalise(ties(), { mode: "RANK" });
    for (let i = 0; i < 43; i += 1) assert.equal(out[i], 21 / 76);
    // Every positive is alone in its tie group, so its mid-rank is its position.
    assert.equal(out[43], 43 / 76);
    assert.equal(out[76], 1);
  },
);

test(
  "QUANTILE on the same input gives the zeros 0, since they are the lowest distinct value (colormaps.py:531-535)",
  () => {
    const out = normalise(ties(), { mode: "QUANTILE" });
    for (let i = 0; i < 43; i += 1) assert.equal(out[i], 0);
    // 35 distinct values, so the top one lands on 1.
    assert.equal(out[76], 1);
    assert.equal(out[43], 1 / 34);
  },
);

test("LINEAR maps the raw range onto 0..1 (colormaps.py:536-538)", () => {
  assert.deepEqual([...normalise([0, 5, 10], { mode: "LINEAR" })], [0, 0.5, 1]);
  // Out-of-range values are clipped to the bounds, not extrapolated (colormaps.py:517,540).
  assert.deepEqual([...normalise([-3, 5, 13], { mode: "LINEAR" })], [0, 0.5, 1]);
});

test("vmin and vmax override the data bounds (colormaps.py:437-438)", () => {
  assert.deepEqual([...normalise([0, 5, 10], { mode: "LINEAR", vmin: 5, vmax: 10 })], [0, 0, 1]);
});

test("a degenerate range is widened by 1e-9, not divided by zero (colormaps.py:439-440)", () => {
  const out = normalise([7, 7, 7], { mode: "LINEAR" });
  assert.ok(Number.isFinite(at(out, 0)));
  assert.equal(at(out, 0), 0);
});

test("a percentile clip of 5..95 bounds the sample, numpy linear interpolation (colormaps.py:423-435)", () => {
  const values = Array.from({ length: 101 }, (_, i) => i);
  const out = normalise(values, { mode: "LINEAR", clipLowPct: 5, clipHighPct: 95 });
  // np.percentile([0..100], 5) = 5.0, np.percentile([0..100], 95) = 95.0.
  assert.equal(out[5], 0);
  assert.equal(out[95], 1);
  assert.equal(out[50], 45 / 90);
});

test(
  "the percentile interpolates between two samples the way numpy's _lerp does (colormaps.py:431-432)",
  () => {
    // n = 11, so 5% sits at (11-1)*0.05 = 0.5 between 0.1 and 0.7 and 95% at 9.5
    // between 55.9 and 89.3. numpy takes b - (b-a)*(1-t) once t >= 0.5, which is
    // not the same double as a + (b-a)*t, so the bounds and the outputs move by
    // an ulp. SciGraphs' own normalize_values on this sample gives these values.
    const values = [0.1, 0.7, 1.3, 2.9, 3.1, 8.8, 13.0, 21.5, 34.2, 55.9, 89.3];
    const out = normalise(values, { mode: "LINEAR", clipLowPct: 5, clipHighPct: 95 });
    assert.equal(at(out, 0), 0);
    assert.equal(at(out, 1), 0.004155124653739612);
    assert.equal(at(out, 2), 0.01246537396121884);
    assert.equal(at(out, 10), 1);
  },
);

test("percentile bounds are clamped into 0..100 before the pass (colormaps.py:426-427)", () => {
  const ramp = Array.from({ length: 101 }, (_, i) => i);
  // -10 clamps to 0 and 150 to 100, so both cases become ordinary clips.
  assert.deepEqual(
    [...normalise(ramp, { mode: "LINEAR", clipLowPct: -10, clipHighPct: 90 })],
    [...normalise(ramp, { mode: "LINEAR", clipLowPct: 0, clipHighPct: 90 })],
  );
  assert.deepEqual(
    [...normalise(ramp, { mode: "LINEAR", clipLowPct: 5, clipHighPct: 150 })],
    [...normalise(ramp, { mode: "LINEAR", clipLowPct: 5, clipHighPct: 100 })],
  );
});

test("reversed percentile bounds are swapped, not inverted (colormaps.py:428-429)", () => {
  const ramp = Array.from({ length: 101 }, (_, i) => i);
  assert.deepEqual(
    [...normalise(ramp, { mode: "LINEAR", clipLowPct: 95, clipHighPct: 5 })],
    [...normalise(ramp, { mode: "LINEAR", clipLowPct: 5, clipHighPct: 95 })],
  );
});

test("the degenerate range is widened by exactly 1e-9, no more and no less (colormaps.py:439-440)", () => {
  // vmin == vmax == 5 makes hi 5 + 1e-9 and the second sample sits 5e-10 above
  // the low bound, so it lands at (5e-10)/(1e-9). A wider nudge shrinks that
  // ratio and a narrower one grows it; either moves the exact quotient below.
  const out = normalise([5, 5 + 5e-10], { mode: "LINEAR", vmin: 5, vmax: 5 });
  const widened = 5 + 1e-9 - 5;
  assert.ok(Math.abs(widened - 1e-9) < 1e-15, `${widened}`);
  assert.equal(at(out, 0), 0);
  assert.equal(at(out, 1), (5 + 5e-10 - 5) / widened);
});

test(
  "LOG with no positive sample is 0 for every node, the epsilon floor standing alone (colormaps.py:446-448)",
  () => {
    // No sample above zero leaves pos_min undefined; the source returns the bare
    // 1e-30 floor, so every value lands on log10(1e-30) == plan_lo, which is 0.
    assert.deepEqual([...normalise([0, 0, 0], { mode: "LOG" })], [0, 0, 0]);
    assert.deepEqual([...normalise([-3, -1, 0], { mode: "LOG" })], [0, 0, 0]);
    assert.deepEqual([...normalise([0], { mode: "LOG" })], [0]);
    // Clipped down to <= 0 by vmax the same floor applies (colormaps.py:517,521).
    assert.deepEqual([...normalise([1, 10, 100], { mode: "LOG", vmin: -2, vmax: 0 })], [0, 0, 0]);
  },
);

test("the LOG epsilon floor is 1e-30 when the solved epsilon falls under it (colormaps.py:383,458-459)", () => {
  // pos_min 1e-28, hi 1: the solved gap is 28*0.1/0.9 decades, so pos_min*10^-gap
  // is 7.7e-32 and the 1e-30 floor takes over. plan_lo is then log10(1e-30) = -30,
  // so 1e-28 lands at (-28 + 30)/30. A 1e-12 floor would clamp it to 0 instead.
  const out = normalise([0, 1e-28, 1], { mode: "LOG" });
  assert.equal(at(out, 0), 0);
  assert.equal(at(out, 1), 0.06666666666666667);
  assert.equal(at(out, 2), 1);
});

test("a million samples normalise, so none of them becomes a call argument (colormaps.py:434-435)", () => {
  // np.nanmin/np.nanmax and positive.min() take an array; a spread would push
  // every sample onto the call stack and throw long before this length.
  const ramp = Array.from({ length: 1_000_000 }, (_, i) => i);
  const linear = normalise(ramp, { mode: "LINEAR" });
  assert.equal(linear.length, 1_000_000);
  assert.equal(at(linear, 0), 0);
  assert.equal(at(linear, 500_000), 500_000 / 999_999);
  assert.equal(at(linear, 999_999), 1);
  // LOG reads the smallest positive sample the same way, through logEpsilon.
  const log = normalise(ramp, { mode: "LOG" });
  assert.equal(at(log, 0), 0);
  assert.equal(at(log, 999_999), 1);
});

test(
  "LOG floors zeros and negatives to the solved epsilon (colormaps.py:382-385,444-459,520-526)",
  () => {
    const out = normalise([0, 1, 10, 100], { mode: "LOG" });
    // eps solves gap = span*f/(1-f) decades from pos_min=1, span=log10(100)=2, f=0.1.
    const gap = (2 * 0.1) / 0.9;
    const eps = Math.max(10 ** -gap, 1e-30);
    assert.equal(out[0], 0);
    assert.equal(out[3], 1);
    assert.equal(out[2], (Math.log10(10) - Math.log10(eps)) / (2 - Math.log10(eps)));
  },
);

test("LOG with no zero or negative is a plain log of the data (colormaps.py:451-453)", () => {
  const out = normalise([1, 10, 100], { mode: "LOG" });
  // eps stays under the data and never binds, so the smallest value sits at 0.
  assert.equal(out[0], 0);
  assert.equal(out[2], 1);
});

test("negatives are floored to the epsilon too, so they share the zero colour (colormaps.py:526)", () => {
  const out = normalise([-5, 0, 1, 100], { mode: "LOG" });
  assert.equal(out[0], 0);
  assert.equal(out[1], 0);
  assert.equal(out[3], 1);
  assert.ok(at(out, 2) > 0 && at(out, 2) < 1);
});

test(
  "the epsilon gap never falls under 0.05 decades, which sets how much ramp the zeros get (colormaps.py:385,458)",
  () => {
    // pos_min 1, hi 1.1: the solved gap is 0.0046 decades, under the 0.05 floor.
    const out = normalise([0, 1, 1.1], { mode: "LOG" });
    const eps = 10 ** -0.05;
    assert.equal(out[0], 0);
    assert.equal(out[1], 0.05 / (Math.log10(1.1) + 0.05));
    assert.equal(out[2], 1);
    assert.ok(eps > 0);
  },
);

test(
  "with no zero to floor, eps is still max(pos_min*1e-3, 1e-30), and the 1e-3 term sets plan_lo (colormaps.py:382,451-453)",
  () => {
    // No value is <= 0, so eps = 1*1e-3 = 1e-3; vmin 1e-5 puts the plan's lo on that
    // epsilon rather than on vmin, which is the term a bare 1e-30 floor would lose.
    const out = normalise([1, 10], { mode: "LOG", vmin: 1e-5 });
    // plan_lo = log10(1e-3), plan_hi = log10(10), so 1 lands at 3/4, not at 5/6.
    assert.equal(out[0], 3 / 4);
    assert.equal(out[1], 1);
  },
);

test("an invalid gamma is 1, a real one is norm^(1/gamma) (colormaps.py:492-493,542-543)", () => {
  assert.deepEqual([...normalise([0, 5, 10], { mode: "LINEAR", gamma: 0 })], [0, 0.5, 1]);
  assert.deepEqual([...normalise([0, 5, 10], { mode: "LINEAR", gamma: Number.NaN })], [0, 0.5, 1]);
  assert.deepEqual([...normalise([0, 5, 10], { mode: "LINEAR", gamma: 2 })], [0, 0.5 ** 0.5, 1]);
});

test("non-finite inputs stay NaN (colormaps.py:504,516)", () => {
  const out = normalise([Number.NaN, 1, Number.POSITIVE_INFINITY], { mode: "LINEAR" });
  assert.ok(Number.isNaN(at(out, 0)));
  assert.ok(Number.isNaN(at(out, 2)));
  assert.equal(at(out, 1), 0);
});

test("an all-non-finite sample is all NaN (colormaps.py:505-510)", () => {
  const out = normalise([Number.NaN, Number.NEGATIVE_INFINITY], { mode: "RANK" });
  assert.deepEqual([...out], [Number.NaN, Number.NaN]);
});

test("an empty input gives an empty result (colormaps.py:497-502)", () => {
  assert.equal(normalise([], { mode: "RANK" }).length, 0);
});

test("a single value normalises to 0 without dividing by zero (colormaps.py:529,534)", () => {
  assert.deepEqual([...normalise([42], { mode: "RANK" })], [0]);
  assert.deepEqual([...normalise([42], { mode: "QUANTILE" })], [0]);
  assert.deepEqual([...normalise([42], { mode: "LOG" })], [0]);
});

test("typed arrays are accepted the same as plain numbers", () => {
  const floats = Float64Array.from([0, 1, 2, 3]);
  const ints = Uint32Array.from([0, 1, 2, 3]);
  assert.deepEqual([...normalise(floats, { mode: "LINEAR" })], [...normalise(ints, { mode: "LINEAR" })]);
});

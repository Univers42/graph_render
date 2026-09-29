/**
 * The normalised value to colour step: linear interpolation across the 32 stops,
 * then one srgbEncode on the way out.
 *   SciGraphs/core/scigraphs_core/coloring/colormaps.py:329-335 (the 32 stops)
 *   SciGraphs/ui/coloring/functions.py:250-270,406-410 (stops at i/(n-1), LINEAR)
 *   SciGraphs/core/scigraphs_core/coloring/colormaps.py:558-582 (clip, then the missing colour)
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { MISSING, coloursOf, sampleColormap } from "../src/colour/colormap.ts";
import { normalise } from "../src/colour/normalise.ts";
import { COLORMAP_STOPS } from "../src/colour/tables.ts";
import type { ColormapName } from "../src/colour/tables.ts";
import { cssOf, srgbEncode } from "../src/colour/srgb.ts";

/** Indexed reads go through these so a wrong index fails the test instead of the type. */
function channel(flat: ArrayLike<number>, index: number): number {
  const value = flat[index];
  assert.ok(value !== undefined, `index ${index} out of range`);
  return value;
}

function entry(palette: readonly string[], index: number): string {
  const value = palette[index];
  assert.ok(value !== undefined, `index ${index} out of range`);
  return value;
}

function stop(name: ColormapName, k: number): readonly [number, number, number] {
  const flat = COLORMAP_STOPS[name];
  return [channel(flat, k * 3), channel(flat, k * 3 + 1), channel(flat, k * 3 + 2)];
}

function lerp(a: number, b: number, f: number): number {
  return a + (b - a) * f;
}

test("t=0 is stop 0 and t=1 is stop 31", () => {
  assert.deepEqual(sampleColormap("inferno", 0), stop("inferno", 0));
  assert.deepEqual(sampleColormap("inferno", 1), stop("inferno", 31));
});

test("t=1/62 is the mean of stops 0 and 1", () => {
  const a = stop("inferno", 0);
  const b = stop("inferno", 1);
  const got = sampleColormap("inferno", 1 / 62);
  assert.deepEqual(got, [lerp(a[0], b[0], 0.5), lerp(a[1], b[1], 0.5), lerp(a[2], b[2], 0.5)]);
});

test("t=21/76 interpolates between stops 8 and 9 by hand", () => {
  const a = stop("inferno", 8);
  const b = stop("inferno", 9);
  // 21/76 sits at (21/76) * 31 = 171/24 of a stop width from stop 8.
  const f = (21 / 76) * 31 - 8;
  const got = sampleColormap("inferno", 21 / 76);
  assert.deepEqual(got, [lerp(a[0], b[0], f), lerp(a[1], b[1], f), lerp(a[2], b[2], f)]);
});

test("a NaN t is the missing colour of colormaps.py:563, (0.30, 0.30, 0.30)", () => {
  assert.deepEqual(MISSING, [0.3, 0.3, 0.3]);
  assert.deepEqual(sampleColormap("inferno", Number.NaN), MISSING);
});

test("every non-finite t is the missing colour, not a clamped end stop (colormaps.py:570-571,582)", () => {
  // np.isfinite is the mask, so an infinity is missing like a NaN; only the
  // finite samples are written into the ramp.
  assert.deepEqual(sampleColormap("inferno", Number.POSITIVE_INFINITY), MISSING);
  assert.deepEqual(sampleColormap("inferno", Number.NEGATIVE_INFINITY), MISSING);
  const { palette, slots } = coloursOf(
    Float64Array.from([Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY, 0.5]),
    "viridis",
  );
  // Both infinities are the one missing colour, so they share slot 0.
  assert.deepEqual([...new Set([...slots])], [0, 1]);
  assert.equal(entry(palette, channel(slots, 0)), cssOf(MISSING));
  assert.equal(entry(palette, channel(slots, 1)), cssOf(MISSING));
  assert.equal(entry(palette, channel(slots, 2)), cssOf(sampleColormap("viridis", 0.5)));
});

test("t outside 0..1 is clamped", () => {
  assert.deepEqual(sampleColormap("viridis", -3), stop("viridis", 0));
  assert.deepEqual(sampleColormap("viridis", 4), stop("viridis", 31));
});

test("43 zero-betweenness nodes share one palette slot whose colour is cssOf(sampleColormap(\"inferno\", 21/76))", () => {
  const raw: number[] = [];
  for (let i = 0; i < 43; i += 1) raw.push(0);
  for (let i = 1; i <= 34; i += 1) raw.push(i);
  const norm = normalise(raw, { mode: "RANK" });
  assert.equal(norm[0], 21 / 76);
  const { palette, slots } = coloursOf(norm, "inferno");
  assert.equal(new Set([...slots]).size, 35);
  const first = channel(slots, 0);
  for (let i = 0; i < 43; i += 1) assert.equal(channel(slots, i), first);
  assert.equal(entry(palette, first), cssOf(sampleColormap("inferno", 21 / 76)));
});

test("the palette holds one entry per distinct colour and the encode runs exactly once", () => {
  const norm = Float64Array.from([0, 0.25, 1]);
  const { palette, slots } = coloursOf(norm, "plasma");
  assert.deepEqual([...slots], [0, 1, 2]);
  assert.equal(palette.length, 3);
  // Encoded once: byte = round(255 * encode(interpolated channel)), not a second pass.
  const linear = sampleColormap("plasma", 0.25);
  const expected = linear.map((c) => Math.round(255 * srgbEncode(c))).join(", ");
  assert.equal(entry(palette, 1), `rgb(${expected})`);
});

test("a NaN norm slot gets the missing colour", () => {
  const { palette, slots } = coloursOf(Float64Array.from([Number.NaN, 0]), "magma");
  assert.equal(entry(palette, channel(slots, 0)), cssOf(MISSING));
  assert.equal(entry(palette, channel(slots, 1)), cssOf(sampleColormap("magma", 0)));
  assert.equal(palette.length, 2);
});

test("the palette is keyed on the colour, so two t values with the same bytes share one entry", () => {
  // 0.5, 0.5+1e-12 and 0.5+2e-12 all encode to rgb(222, 128, 156), so the three
  // nodes draw through one palette entry however the lookup is keyed.
  const norm = Float64Array.from([0.5, 0.5 + 1e-12, 0.5 + 2e-12]);
  const { palette, slots } = coloursOf(norm, "inferno");
  assert.equal(palette.length, 1);
  assert.deepEqual([...slots], [0, 0, 0]);
  assert.equal(entry(palette, 0), "rgb(222, 128, 156)");
});

test("more than 1024 distinct values are quantised onto the 1024-step t grid, to the nearest step", () => {
  // 5000 values inside 0..1 that all sit off the grid, plus one exactly half a
  // step above grid step 1. Math.round breaks that tie upwards, so it snaps to
  // 2/1023, and cividis reads one byte apart at the two.
  const values: number[] = [];
  for (let i = 0; i < 5000; i += 1) values.push(i / 4999);
  values.push(1.5 / 1023);
  assert.ok(new Set(values).size > 1024);
  const { palette, slots } = coloursOf(Float64Array.from(values), "cividis");
  assert.ok(palette.length <= 1024, `palette ${palette.length}`);
  assert.equal(entry(palette, channel(slots, 5000)), cssOf(sampleColormap("cividis", 2 / 1023)));
});

test("at most 1024 distinct values are not quantised at all", () => {
  const norm = new Float64Array(1024);
  for (let i = 0; i < 1024; i += 1) norm[i] = (i + 0.5) / 1024;
  assert.equal(new Set(norm).size, 1024);
  const { palette, slots } = coloursOf(norm, "cividis");
  // Every entry is its own t unsnapped: rounding onto the 1/1023 grid would move
  // most of these by half a step or more.
  for (let i = 0; i < 1024; i += 1) {
    assert.equal(entry(palette, channel(slots, i)), cssOf(sampleColormap("cividis", norm[i] ?? 0)));
  }
  // One entry per distinct colour, so the palette is shorter than the sample.
  assert.equal(palette.length, new Set(palette).size);
  assert.ok(palette.length < 1024, `${palette.length}`);
});

test("no node at all gives an empty palette and no slots", () => {
  const { palette, slots } = coloursOf(new Float64Array(0), "turbo");
  assert.deepEqual([...palette], []);
  assert.equal(slots.length, 0);
});

test("the named colormaps of the presets all resolve", () => {
  for (const name of ["inferno", "viridis", "plasma", "cividis", "magma", "turbo"] as const) {
    assert.equal(sampleColormap(name, 0.5).length, 3, name);
  }
  assert.ok(COLORMAP_STOPS.inferno.length === 96);
});

/** The generated 32-stop ramps must be the pinned matplotlib table, sampled the way matplotlib does. */
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";

import { COLORMAP_NAMES, COLORMAP_STOPS } from "../src/colour/tables.ts";
import { sampleColormap } from "../src/colour/colormap.ts";

const REFERENCE = "/refs/matplotlib-3.10.0/_cm_listed.py";
const mounted = existsSync(REFERENCE);
const skipReason = "pinned matplotlib reference not mounted";

function rowsOf(name: string): number[][] | null {
  if (!mounted) return null;
  const text = readFileSync(REFERENCE, "utf8");
  const start = text.indexOf(`_${name}_data = [`);
  assert.ok(start >= 0, `reference has no _${name}_data`);
  const end = text.indexOf("\n_", start + 1);
  assert.ok(end > 0, `reference has no table after _${name}_data`);
  const block = text.slice(start, end);
  return [...block.matchAll(/\[([^\][]*)\]/g)].map((match) =>
    (match.at(1) ?? "").split(",").map((part) => Number(part.trim())),
  );
}

function rowAt(rows: number[][], index: number): number[] {
  const row = rows.at(index);
  assert.ok(row, `row ${index} missing from the reference`);
  return row;
}

test("every generated map carries 32 stops of r,g,b", () => {
  assert.deepEqual([...COLORMAP_NAMES], [
    "inferno", "viridis", "plasma", "cividis", "magma", "turbo",
  ]);
  for (const name of COLORMAP_NAMES) {
    assert.equal(COLORMAP_STOPS[name].length, 96, `${name} stop count`);
  }
});

test(
  "inferno stop 0 is row 0, stop 31 is row 255 and stop 15 is row trunc(15/31*256)=123 of _cm_listed.py",
  { skip: mounted ? false : skipReason },
  () => {
    const rows = rowsOf("inferno");
    assert.ok(rows);
    assert.equal(rows.length, 256);
    const stops = COLORMAP_STOPS.inferno;
    assert.deepEqual(stops.slice(0, 3), rowAt(rows, 0));
    assert.deepEqual(stops.slice(93, 96), rowAt(rows, 255));
    // matplotlib's Colormap.__call__ for a float: trunc(x * 256), and 256 folds to 255.
    assert.equal(Math.trunc((15 / 31) * 256), 123);
    assert.deepEqual(stops.slice(45, 48), rowAt(rows, 123));
  },
);

test(
  "every stop of every map is the reference row min(255, trunc(k/31*256)), _cm_listed.py:260",
  { skip: mounted ? false : skipReason },
  () => {
    // Stops 0, 15 and 31 agree under trunc(x*256), round(x*255) and floor(x*255)
    // alike, so the whole table is compared: k = 3 tells the rules apart (24 or
    // 25), k = 24 (198 or 197) and k = 28 (231 or 230).
    for (const name of COLORMAP_NAMES) {
      const rows = rowsOf(name);
      assert.ok(rows);
      for (let k = 0; k < 32; k += 1) {
        const index = Math.min(255, Math.trunc((k / 31) * 256));
        assert.deepEqual(COLORMAP_STOPS[name].slice(k * 3, k * 3 + 3), rowAt(rows, index), `${name} stop ${k}`);
      }
    }
  },
);

test(
  "the ramp at t = 21/76 is the reference rows 66 and 74 interpolated, _cm_listed.py:260",
  { skip: mounted ? false : skipReason },
  () => {
    // 21/76 * 31 = 8.5658.. of a stop width, so it falls between stop 8
    // (trunc(8/31*256) = 66) and stop 9 (trunc(9/31*256) = 74).
    const rows = rowsOf("inferno");
    assert.ok(rows);
    const a = rowAt(rows, 66);
    const b = rowAt(rows, 74);
    const f = (21 / 76) * 31 - 8;
    const got = sampleColormap("inferno", 21 / 76);
    assert.deepEqual(
      got,
      [0, 1, 2].map((c) => {
        const from = a[c] ?? 0;
        const to = b[c] ?? 0;
        return from + (to - from) * f;
      }),
    );
  },
);

test(
  "each map's first stop is its own first row and its last stop is its own last row",
  { skip: mounted ? false : skipReason },
  () => {
    for (const name of COLORMAP_NAMES) {
      const rows = rowsOf(name);
      assert.ok(rows);
      assert.deepEqual(COLORMAP_STOPS[name].slice(0, 3), rowAt(rows, 0));
      assert.deepEqual(COLORMAP_STOPS[name].slice(93, 96), rowAt(rows, 255));
    }
  },
);

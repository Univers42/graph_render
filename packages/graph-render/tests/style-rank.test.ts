/**
 * `rankByWeight` against the comparator sort it replaced, element for element, and the two
 * one-entry caches `styleFrom` now keeps.
 */
import assert from "node:assert/strict";
import { test } from "node:test";
import { isDeepStrictEqual } from "node:util";

import { rankByWeight } from "../src/rank.ts";
import { radiusFor, type Sizing, type StyleInput, styleFrom } from "../src/style.ts";
import { mulberry32 } from "./support.ts";

/** The oracle: `rankOf` as it stood in `src/style.ts:120-124` at 3c7c2c4e. */
function comparatorRank(weights: Float32Array): Uint32Array {
  const rank = new Uint32Array(weights.length);
  for (let i = 0; i < rank.length; i += 1) rank[i] = i;
  return rank.sort((a, b) => (weights[b] ?? 0) - (weights[a] ?? 0) || a - b);
}

/** The same sort with ties in reverse index order: what an unstable rank could produce. */
function reversedTies(weights: Float32Array): Uint32Array {
  const rank = new Uint32Array(weights.length);
  for (let i = 0; i < rank.length; i += 1) rank[i] = i;
  return rank.sort((a, b) => (weights[b] ?? 0) - (weights[a] ?? 0) || b - a);
}

/** The one check every oracle case runs, so the negative control exercises the same check. */
function agrees(rank: Uint32Array, weights: Float32Array): boolean {
  return isDeepStrictEqual(rank, comparatorRank(weights));
}

/** `count` seeded weights in 0..1; a non-zero `steps` quantises them to 1/steps, so ties abound. */
function seeded(count: number, seed: number, steps = 0): Float32Array {
  const next = mulberry32(seed);
  return Float32Array.from({ length: count }, () => (steps > 0 ? Math.round(next() * steps) / steps : next()));
}

const TIES = seeded(1000, 7, 8);

const ORACLE_CASES: readonly (readonly [string, Float32Array])[] = [
  ["n = 0", new Float32Array(0)],
  ["n = 1", Float32Array.from([0.3])],
  ["n = 2, lighter first", Float32Array.from([0.1, 0.9])],
  ["n = 2, tied", Float32Array.from([0.5, 0.5])],
  ["all weights equal", new Float32Array(257).fill(0.25)],
  ["mixed +0 and -0", Float32Array.from([0, -0, 0.5, -0, 0, -0])],
  ["±Infinity", Float32Array.from([1, Infinity, -Infinity, 0, Infinity, -1, -Infinity])],
  ["negatives and subnormals", Float32Array.from([-0.5, 1e-45, -1e-45, 3.4e38, -3.4e38, 0, 2e-40, -2e-40])],
  ["quantised ties", TIES],
  ["a seeded 100 000", seeded(100_000, 20261003)],
];

for (const [name, weights] of ORACLE_CASES) {
  test(`oracle: rankByWeight equals the comparator sort, ${name}`, () => {
    assert.deepEqual(rankByWeight(weights), comparatorRank(weights));
  });
}

test("a NaN ranks where a +0 at the same index would", () => {
  const withNaN = Float32Array.from([0.5, 0, 0.25, Number.NaN, 0, 1, Number.NaN]);
  const withZero = withNaN.map((weight) => (Number.isNaN(weight) ? 0 : weight));
  assert.deepEqual(rankByWeight(withNaN), comparatorRank(withZero));
});

test("negative control: ties in reverse index order fail the oracle check", () => {
  assert.equal(agrees(rankByWeight(TIES), TIES), true);
  assert.equal(agrees(reversedTies(TIES), TIES), false);
});

function input(weights: Float32Array, patch: Partial<StyleInput> = {}): StyleInput {
  return { labels: [], weights, colours: new Uint16Array(weights.length), palette: ["a", "b", "c"], ...patch };
}

test("radius: byte-equal to the per-node clamped radiusFor loop, and the same maxRadius", () => {
  const weights = seeded(10_000, 11).map((weight, i) => (i % 97 === 0 ? Number.NaN : weight * 1.4 - 0.2));
  const sizing: Sizing = { base: 3, gain: 2, min: 4, max: 8 };
  // `clamped` as `src/style.ts` writes it, per node, before the f32 store rounds it.
  const values = Array.from(weights, (weight) => Math.min(sizing.max ?? Infinity, Math.max(sizing.min ?? 0, radiusFor(weight, sizing))));
  const expected = Float32Array.from(values);
  const style = styleFrom(input(weights, { sizing }));
  assert.deepEqual(new Uint8Array(style.radius.buffer), new Uint8Array(expected.buffer));
  assert.equal(style.maxRadius, values.reduce((most, radius) => Math.max(most, radius), 0));
});

test("memo: the same weights and sizing hand back the same rank and radius, whatever the mask", () => {
  const weights = seeded(500, 3);
  const first = styleFrom(input(weights, { sizing: { base: 4, gain: 2.5 } }));
  const step = styleFrom(input(weights, { sizing: { base: 4, gain: 2.5 }, hidden: new Uint8Array(500).fill(1) }));
  assert.equal(step.rank, first.rank);
  assert.equal(step.radius, first.radius);
  assert.equal(step.maxRadius, first.maxRadius);
  assert.equal(step.hidden?.length, 500);
});

test("memo: a new weights array with the same contents is rebuilt to equal values", () => {
  const weights = seeded(500, 4, 16);
  const first = styleFrom(input(weights));
  const again = styleFrom(input(weights.slice()));
  assert.notEqual(again.rank, first.rank);
  assert.deepEqual(again.rank, first.rank);
  assert.deepEqual(again.radius, first.radius);
});

test("memo: a changed sizing gain gives a new radius", () => {
  const weights = seeded(500, 5);
  const first = styleFrom(input(weights, { sizing: { base: 4, gain: 2.5 } }));
  const regained = styleFrom(input(weights, { sizing: { base: 4, gain: 1 } }));
  assert.notEqual(regained.radius, first.radius);
  assert.notDeepEqual(regained.radius, first.radius);
  assert.deepEqual(regained.rank, first.rank);
});

test("memo: the same colours and palette length hand back the same buckets", () => {
  const colours = Uint16Array.from({ length: 300 }, (_, i) => (i * 7) % 3);
  const weights = seeded(300, 6);
  const first = styleFrom(input(weights, { colours }));
  const step = styleFrom(input(weights, { colours, hidden: new Uint8Array(300) }));
  assert.equal(step.bucketStart, first.bucketStart);
  assert.equal(step.bucketItems, first.bucketItems);
});

test("memo: new colours with the same contents are rebuilt equal, and a new palette length re-buckets", () => {
  const colours = Uint16Array.from({ length: 300 }, (_, i) => (i * 5) % 3);
  const weights = seeded(300, 8);
  const first = styleFrom(input(weights, { colours }));
  const again = styleFrom(input(weights, { colours: colours.slice() }));
  assert.notEqual(again.bucketItems, first.bucketItems);
  assert.deepEqual(again.bucketStart, first.bucketStart);
  assert.deepEqual(again.bucketItems, first.bucketItems);
  const narrower = styleFrom(input(weights, { colours: again.colours, palette: ["a", "b"] }));
  assert.notEqual(narrower.bucketStart, again.bucketStart);
  assert.equal(narrower.bucketStart.length, 3);
});

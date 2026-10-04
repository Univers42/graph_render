import assert from "node:assert/strict";
import { test } from "node:test";

import { MAX_SPREAD, TARGET_SPACING, boundsOf, readableFactor, worldFactor } from "../src/frame.ts";
import { typicalSpacing } from "../src/spacing.ts";
import { mulberry32 } from "./support.ts";

interface Points {
  readonly x: Float32Array;
  readonly y: Float32Array;
}

/** A side × side lattice at `pitch` from the origin, then `extra` points after it. */
function lattice(side: number, pitch: number, extra: readonly (readonly [number, number])[] = []): Points {
  const count = side * side;
  const x = new Float32Array(count + extra.length);
  const y = new Float32Array(count + extra.length);
  for (let i = 0; i < count; i += 1) {
    x[i] = (i % side) * pitch;
    y[i] = Math.floor(i / side) * pitch;
  }
  extra.forEach(([px, py], at) => {
    x[count + at] = px;
    y[count + at] = py;
  });
  return { x, y };
}

/** `count` points evenly on a circle of `radius` around the origin. */
function circle(count: number, radius: number): (readonly [number, number])[] {
  return Array.from({ length: count }, (_, k): readonly [number, number] => {
    const angle = (2 * Math.PI * k) / count;
    return [radius * Math.cos(angle), radius * Math.sin(angle)];
  });
}

/** Today's factor (the floor) and the new one, over the same columns and node hull. */
function factors(points: Points): { readonly floor: number; readonly readable: number } {
  const bounds = boundsOf(points.x, points.y);
  return { floor: worldFactor(bounds, points.x.length), readable: readableFactor(points.x, points.y, bounds) };
}

test("a uniform lattice keeps today's factor", () => {
  const points = lattice(10, 1);
  // The lattice's box is 9 × 9, so the median node, in a full cell of 16, reads
  // sqrt(81 / 100) = 0.9: the very spacing the floor reads, to the bit.
  const spacing = typicalSpacing(points.x, points.y);
  assert.ok(Math.abs(spacing - 0.9) < 1e-6, `spacing ${spacing}`);
  const { floor, readable } = factors(points);
  assert.equal(readable, floor);
  assert.ok(Math.abs(floor - TARGET_SPACING / 0.9) < 1e-9, `floor ${floor}`);
});

test("a dense clump with far outliers is spread to a readable pitch", () => {
  // 900 nodes at pitch 0.01 and 100 on a circle of radius 10: the box is the circle's, so
  // sqrt(area / n) reads a spacing 63 times the clump's pitch.
  const { floor, readable } = factors(lattice(30, 0.01, circle(100, 10)));
  assert.ok(floor * 0.01 < 2, `today's pitch ${floor * 0.01} is not the defect`);
  assert.ok(readable * 0.01 >= TARGET_SPACING / 2, `the clump's pitch ${readable * 0.01} is under ${TARGET_SPACING / 2}`);
});

test("nodes all on one point have no spacing and keep the floor", () => {
  const points = { x: new Float32Array(50).fill(3), y: new Float32Array(50).fill(3) };
  assert.equal(typicalSpacing(points.x, points.y), 0);
  assert.deepEqual(factors(points), { floor: 1, readable: 1 });
});

test("a collinear row at pitch 1 is brought to the target spacing", () => {
  const points = { x: Float32Array.from({ length: 10 }, (_, i) => i), y: new Float32Array(10).fill(5) };
  assert.equal(factors(points).readable, TARGET_SPACING);
});

test("the same columns give the same factor, to the bit", () => {
  const points = lattice(30, 0.01, circle(100, 10));
  assert.equal(typicalSpacing(points.x, points.y), typicalSpacing(points.x, points.y));
  assert.equal(factors(points).readable, factors(points).readable);
});

test("a uniform random spread of 200 000 nodes barely moves", () => {
  const random = mulberry32(42);
  const n = 200_000;
  const points = {
    x: Float32Array.from({ length: n }, () => random() * 1000),
    y: Float32Array.from({ length: n }, () => random() * 1000),
  };
  const { floor, readable } = factors(points);
  const ratio = readable / floor;
  assert.ok(ratio >= 1 && ratio <= 1.25, `readable / floor is ${ratio}`);
});

test("a clump cannot spread a drawing past MAX_SPREAD times the floor", () => {
  const onePoint = { x: new Float32Array(1000), y: new Float32Array(1000) };
  onePoint.x[999] = 1e6;
  onePoint.y[999] = 1e6;
  const single = factors(onePoint);
  assert.ok(single.readable <= single.floor * MAX_SPREAD, `${single.readable} > ${single.floor} * ${MAX_SPREAD}`);
  // A clump with a spacing of its own, 1e-3, would ask for 56 000: the clamp binds.
  const tight = factors(lattice(31, 1e-3, [[1e6, 1e6]]));
  assert.equal(tight.readable, tight.floor * MAX_SPREAD);
});

// The rate the view reports: frames painted while it moves, and nothing else.
import assert from "node:assert/strict";
import { test } from "node:test";

import { fpsOf, newRate, stamp } from "../src/canvas2d/rate.ts";

const FRAME = 1000 / 60;

function moved(times: readonly number[]): ReturnType<typeof newRate> {
  const rate = newRate();
  for (const time of times) stamp(rate, time, true);
  return rate;
}

test("sixty moving frames a second read as sixty", () => {
  const times = Array.from({ length: 30 }, (_, at) => 1000 + at * FRAME);
  const fps = fpsOf(moved(times), 1000 + 29 * FRAME + 1);
  assert.ok(Math.abs(fps - 60) < 0.01, `${fps}`);
});

test("a slow view reads slow: five frames 200 ms apart are five a second", () => {
  const fps = fpsOf(moved([1000, 1200, 1400, 1600, 1800]), 1801);
  assert.ok(Math.abs(fps - 5) < 0.01, `${fps}`);
});

test("a frame painted while nothing moves is not part of the rate", () => {
  const rate = newRate();
  stamp(rate, 1000, false);
  stamp(rate, 1220, false);
  assert.equal(fpsOf(rate, 1221), 0, "two hovers 220 ms apart are not 4.5 frames a second");
});

test("one frame is not a rate", () => {
  assert.equal(fpsOf(moved([1000]), 1001), 0);
});

test("a view parked for longer than 400 ms has no rate", () => {
  assert.equal(fpsOf(moved([1000, 1016, 1033]), 1500), 0);
});

test("frames older than a second are not counted", () => {
  const old = [100, 116, 133];
  const recent = [2000, 2000 + FRAME, 2000 + 2 * FRAME];
  const fps = fpsOf(moved([...old, ...recent]), 2000 + 2 * FRAME + 1);
  assert.ok(Math.abs(fps - 60) < 0.01, `${fps}`);
});

test("the ring holds the newest frames once it has gone round", () => {
  const times = Array.from({ length: 100 }, (_, at) => 1000 + at * FRAME);
  const fps = fpsOf(moved(times), 1000 + 99 * FRAME + 1);
  assert.ok(Math.abs(fps - 60) < 0.01, `${fps}`);
});

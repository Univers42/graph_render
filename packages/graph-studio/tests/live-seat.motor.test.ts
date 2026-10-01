// The live session against the real wasm module: it starts from the picture on screen. On
// 2026-10-01 it started from the motor's own spiral instead, so the first drag on any
// finished layout replaced the whole drawing with that spiral.
import assert from "node:assert/strict";
import { test } from "node:test";

import { decodeSnapshot } from "../../graph-render/src/snapshot/decode.ts";
import { DEFAULT_KNOBS } from "../src/motor/live.ts";
import { ALPHA_MIN } from "../src/motor/liveLoop.ts";
import { FIXTURES_URL, SKIP, realSession } from "./motor.ts";

const VAULT = { kind: "synthetic", seed: 1, nodes: 60, degree: 2, shape: "vault" } as const;

function assertDrawn(port: { positions(): { xs: Float64Array; ys: Float64Array } }, bytes: Uint8Array, what: string): void {
  const drawn = decodeSnapshot(bytes);
  const { xs, ys } = port.positions();
  assert.deepEqual(Array.from(xs), Array.from(drawn.x), `${what}: x`);
  assert.deepEqual(Array.from(ys), Array.from(drawn.y), `${what}: y`);
}

test("the live session starts where the layout drew, and follows the next layout", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  await session.load(VAULT, FIXTURES_URL);
  const radial = await session.layout("layout.circular.radial", null);
  const port = session.forces();
  assert.ok(port !== null);
  assertDrawn(port, radial.bytes, "the first session");
  const grid = await session.layout("layout.grid", null);
  assert.equal(session.forces(), port, "one port per graph, so the loop is not replaced");
  assertDrawn(port, grid.bytes, "after the next layout");
});

function widthOf(xs: Float64Array): number {
  let [low, high] = [Infinity, -Infinity];
  for (const x of xs) [low, high] = [Math.min(low, x), Math.max(high, x)];
  return high - low;
}

// On 2026-10-01 Animate restarted from the random layout's unit square: every node within
// `distance_min` of the others, so the first ticks flung the drawing out to 55 000 units.
test("Animate restarts from the motor's seed, and the settle stays on screen", { skip: SKIP }, async () => {
  const session = realSession();
  await session.open("unused");
  await session.load(VAULT, FIXTURES_URL);
  await session.layout("layout.grid", null);
  const port = session.forces();
  assert.ok(port?.shuffle !== undefined);
  port.setParams(DEFAULT_KNOBS);
  port.shuffle();
  const seed = port.positions().xs.slice();
  let [alpha, widest] = [1, widthOf(seed)];
  while (alpha >= ALPHA_MIN) {
    alpha = port.step(3);
    widest = Math.max(widest, widthOf(port.positions().xs));
  }
  assert.ok(widest < 10 * widthOf(seed), `widest ${widest} against a seed ${widthOf(seed)} wide`);
  port.shuffle();
  assert.deepEqual(Array.from(port.positions().xs), Array.from(seed), "not from where it stopped");
});

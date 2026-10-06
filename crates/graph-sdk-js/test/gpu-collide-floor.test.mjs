// The collide pass's host halves, node-only and with no browser: the CPU's `resolve` in f64
// (`hostCollide`) against the fixture's own `delta_collide`, the f32-narrowing floor
// (`collideFloor`) against the numpy analysis, and the guard's fourth argument (Amendment 3).
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-collide-floor.test.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { collideGuard, collideVerdict } from "../src/gpu/bounds-collide.ts";
import { collideFloor, hostCollide } from "../src/gpu/collide-host.ts";
import { gridFor } from "../src/gpu/collide.ts";
import { loadFixture } from "../src/gpu/fixture.ts";

const FIXTURES = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "fixtures", "gpu");

/** One committed `.gmfx` as an `ArrayBuffer`. */
function fixture(name) {
  const bytes = readFileSync(join(FIXTURES, name));
  return loadFixture(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
}

/** The max absolute difference between `hostCollide`'s delta and the fixture's, over both axes. */
function reproduce(f, grid) {
  const got = hostCollide(f.posX, f.posY, grid);
  let maxAbs = 0;
  for (let i = 0; i < f.n; i += 1) {
    maxAbs = Math.max(maxAbs, Math.abs(got.x[i] - f.delta.collide.x[i]), Math.abs(got.y[i] - f.delta.collide.y[i]));
  }
  return maxAbs;
}

test("the_host_collide_reproduces_the_fixture", () => {
  for (const name of ["mesh-1k-start.gmfx", "mesh-1k-settled.gmfx"]) {
    const f = fixture(name);
    const maxAbs = reproduce(f, gridFor(f.posX, f.posY));
    assert.ok(maxAbs <= 1e-12, `${name}: hostCollide's maxAbs ${maxAbs} is over 1e-12`);
  }
});

test("the_collide_floor_is_the_narrowing_alone", () => {
  // Three nodes whose coordinates are exact in f32: narrowing changes nothing, so the floor is 0.
  const posX = Float64Array.from([0, 10, 20]);
  const posY = Float64Array.from([0, 10, 20]);
  assert.equal(collideFloor(posX, posY, gridFor(posX, posY)), 0, "f32-exact coordinates have no floor");
  // The committed 1k-start fixture: the floor is the numpy (b) column's 6.893e-6, within 5%.
  const f = fixture("mesh-1k-start.gmfx");
  const floor = collideFloor(f.posX, f.posY, gridFor(f.posX, f.posY));
  assert.ok(Math.abs(floor - 6.893e-6) / 6.893e-6 <= 0.05, `mesh-1k-start: floor ${floor} is not within 5% of 6.893e-6`);
});

test("the_1m_start_fails_without_its_floor", () => {
  // The measured 1M-start numbers. `software` has no 1M ceiling row, so only the guard applies.
  const base = {
    n: 1_000_000,
    state: 0,
    arm: "software",
    rmsRel: 7.505127e-4,
    rmsRef: 0.319593,
    k_c: 8,
    reach: 32,
    maxAbs: 1.61097e-3,
    repeatEqual: true,
    order: null,
  };
  const without = collideVerdict({ ...base, floorRms: 0 });
  assert.equal(without.pass, false, "the 1M-start arm fails without its floor");
  assert.match(without.failures.join(" "), /^guard \(n=1000000, state=0\)/, `got ${without.failures}`);
  const withFloor = collideVerdict({ ...base, floorRms: 2.399e-4 });
  assert.deepEqual(withFloor.failures, [], `the 1M-start arm passes at its floor: ${withFloor.failures}`);
});

test("the_guard_names_the_floor_it_was_given", () => {
  // The failure text names the floor, the crowd and the reference it was built from.
  const guard = collideGuard(8, 32, 0.319593, 2.399e-4);
  const verdict = collideVerdict({
    n: 10, state: 0, arm: "hardware", rmsRel: guard * 2, rmsRef: 0.319593, k_c: 8,
    reach: 32, maxAbs: 1, floorRms: 2.399e-4, repeatEqual: true, order: null,
  });
  assert.equal(verdict.pass, false, "a breached guard fails the case");
  assert.match(
    verdict.failures.join(" "),
    /guard \(n=10, state=0\): rmsRel .* over 1e-4 \+ \(floor=0\.0002399 \+ k_c=8·5·2⁻²³·16\)\/0\.319593/,
    `got ${verdict.failures}`,
  );
});

// The TS arm's half of the same file `graph-cli emit-gpu-fixtures` writes: the loader reads
// the committed 1k pair and must report what the emitter wrote, and must refuse a version it
// does not know.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-fixture.test.mjs
//
// No browser and no WebGPU here, so this is a real gate row and not a probe. Every assertion
// is against bytes on disk — nothing is recomputed, because the point of the loader is that
// it does not compute.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";
import { test } from "node:test";

import { loadFixture, scaleFor } from "../src/gpu/fixture.ts";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = join(HERE, "..", "..", "..");
const FIXTURES = join(REPO, "fixtures", "gpu");

const read = (name) => {
  const bytes = readFileSync(join(FIXTURES, name));
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
};

// --- the loader agrees with the emitter on every header field ---

test("the_loader_agrees_on_every_header_field", () => {
  for (const [name, state] of [
    ["mesh-1k-start.gmfx", 0],
    ["mesh-1k-settled.gmfx", 1],
  ]) {
    const f = loadFixture(read(name));
    assert.equal(f.n, 1000, `${name}: n`);
    assert.equal(f.side, 128, `${name}: P is side_for(1000)`);
    assert.equal(f.state, state, `${name}: state`);
    assert.ok(f.m > 0 && f.m < 2000, `${name}: m is about 1.55n, got ${f.m}`);
    assert.ok(f.h > 0, `${name}: h is positive`);
    assert.equal(f.step % 1, 0, `${name}: the rung is an integer`);
    // `**` and libm's `exp2` can differ by one ULP, so this is a closeness check and not
    // an equality: the file carries `h`, and the relation being pinned is that it is the
    // rung's own cell size.
    const rung = 2 ** (f.step / 4);
    assert.ok(Math.abs(f.h - rung) <= 2 * Number.EPSILON * rung, `${name}: h is 2^(step/4)`);
    assert.ok(f.cells >= f.reach && f.cells > 0, `${name}: cells and reach`);
    assert.ok(f.cells <= f.side, `${name}: cells fit in P`);
    for (const pass of ["link", "charge", "collide"]) {
      assert.equal(f.delta[pass].x.length, f.n, `${name}: ${pass} x is one per node`);
      assert.equal(f.delta[pass].y.length, f.n, `${name}: ${pass} y is one per node`);
      assert.ok(
        f.delta[pass].x.some((v) => v !== 0) || f.delta[pass].y.some((v) => v !== 0),
        `${name}: ${pass} moved something`,
      );
    }
    assert.ok(f.edgeLo.every((lo, e) => lo < f.edgeHi[e]), `${name}: no self-loop survives`);
    assert.equal(f.twiddleRe.length, f.side, `${name}: one twiddle per line sample`);
    assert.equal(f.spectrumRe.length, f.side * f.side, `${name}: the spectrum is P·P`);
  }
});

// --- version strictness ---

test("the_loader_refuses_a_version_it_does_not_know", () => {
  const bytes = read("mesh-1k-start.gmfx");
  // `new Uint8Array(buffer)` is a *view*, not a copy: without the slice the two cases
  // below would alias one buffer and the second would inherit the first's major.
  const bumped = new Uint8Array(bytes.slice(0));
  new DataView(bumped.buffer).setUint32(4, 2, true);
  assert.throws(() => loadFixture(bumped.buffer), /format major 2/, "a future major throws");
  // And a *newer minor* is read, not refused: the minor only adds sections after the ones
  // named in the README, so the offsets this reader uses are unchanged.
  const minor = new Uint8Array(bytes.slice(0));
  new DataView(minor.buffer).setUint32(8, 1, true);
  assert.equal(loadFixture(minor.buffer).n, 1000, "a newer minor still reads");
});

// --- the scale the deposit needs, derived not stored ---

test("the_two_scales_agree", () => {
  // Rust's `scale_for` is `2^(31 - ceil(log2 n))` in integer arithmetic. The values here
  // are what `emit.rs` computes for the same `n`; `the_two_scales_agree` in
  // `gpu_fixtures/tests.rs` pins the Rust half, and this pins the TypeScript half against
  // the same table. `n = 1` is in it because that is the one case where a signed `<<` is a
  // single bit from correct rather than visibly wrong.
  const table = new Map([
    [1, 1073741824],
    [2, 536870912],
    [1000, 2097152],
    [10000, 131072],
    [50000, 32768],
    [1000000, 2048],
  ]);
  for (const [n, expected] of table) {
    assert.equal(scaleFor(n), expected, `n ${n}: both arms say ${expected}`);
  }
  assert.equal(scaleFor(0), undefined, "no nodes, no scale, on either arm");
  // And the fixture's own header agrees with the scale its node count implies.
  assert.equal(scaleFor(loadFixture(read("mesh-1k-start.gmfx")).n), 2097152, "the 1k pair");
});

// --- refusals ---

test("the_loader_refuses_what_is_not_a_fixture", () => {
  assert.throws(() => loadFixture(new ArrayBuffer(16)), /shorter than/, "a short buffer");
  const bytes = read("mesh-1k-start.gmfx");
  new DataView(bytes).setUint32(0, 0, true);
  assert.throws(() => loadFixture(bytes), /magic/, "a wrong magic");
  const truncated = read("mesh-1k-start.gmfx").slice(0, 1000);
  assert.throws(() => loadFixture(truncated), /ends at/, "a truncated payload");
});

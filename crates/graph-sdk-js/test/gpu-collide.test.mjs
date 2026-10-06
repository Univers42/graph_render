// The collide pass's host halves, node-only and with no browser: the hash's cells and buckets
// against the CPU's formula, the scan's three dispatches against a plain prefix sum, the order
// check, and the ceiling table against its guard. The WGSL itself runs only in the browser arm.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-collide.test.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { cellOf, gridFor } from "../src/gpu/collide.ts";
import { loadFixture } from "../src/gpu/fixture.ts";

const FIXTURES = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..", "fixtures", "gpu");

/** One committed `.gmfx` as an `ArrayBuffer`. */
function fixture(name) {
  const bytes = readFileSync(join(FIXTURES, name));
  return loadFixture(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength));
}

/** `collide/hash.rs`'s bucket, in `u64` arithmetic: `(row(cy) + cx) & mask`. */
function cpuBucket(cx, cy, buckets) {
  const shift = 64n - BigInt(Math.log2(buckets));
  const row = BigInt.asUintN(64, BigInt.asUintN(64, BigInt(cy)) * 0x9e3779b97f4a7c15n) >> shift;
  return Number(BigInt.asUintN(64, row + BigInt.asUintN(64, BigInt(cx))) & BigInt(buckets - 1));
}

/** The minimum of the finite `f64` positions, `frame::bounds`'s `lo` (`collide.rs:146`). */
function cpuOrigin(values) {
  return values.reduce((lo, v) => (Number.isFinite(v) && v < lo ? v : lo), Infinity);
}

test("the_collide_hash_matches_the_cpu_s_cells", () => {
  for (const name of ["mesh-1k-start.gmfx", "mesh-1k-settled.gmfx"]) {
    const f = fixture(name);
    const grid = gridFor(f.posX, f.posY);
    const ox = cpuOrigin(f.posX);
    const oy = cpuOrigin(f.posY);
    // The collide diameter at the frozen parameters, 2 · 16 (params.rs:71), is the cell size.
    assert.equal(grid.reach, 32, `${name}: the cell is one collide diameter`);
    // The kernel's lookup, `(rows[cy + 1] + u32(cx)) & mask`: `&` wraps the sum to 32 bits.
    const device = (cx, cy) => {
      const row = grid.rows[cy + 1];
      assert.ok(row !== undefined, `${name}: row ${cy} is outside the ${grid.rows.length}-row table`);
      return (row + cx) & grid.mask;
    };
    for (let i = 0; i < f.n; i += 1) {
      const cx = Math.trunc((f.posX[i] - ox) / 32);
      const cy = Math.trunc((f.posY[i] - oy) / 32);
      const gx = cellOf(Math.fround(f.posX[i]), grid.originX, grid.invSize);
      const gy = cellOf(Math.fround(f.posY[i]), grid.originY, grid.invSize);
      assert.deepEqual([gx, gy], [cx, cy], `${name}: node ${i}'s f32 cell is the CPU's`);
      // The node's own bucket, and the three rows a query from its cell reads.
      for (const [qx, qy] of [[cx, cy], [cx - 1, cy - 1], [cx - 1, cy], [cx - 1, cy + 1]]) {
        assert.equal(
          device(qx, qy),
          cpuBucket(qx, qy, grid.buckets),
          `${name}: node ${i}, cell (${qx}, ${qy}): the row table's bucket is the u64 hash's`,
        );
      }
    }
  }
});

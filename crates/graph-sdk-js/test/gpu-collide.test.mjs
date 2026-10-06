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

import { compare, components } from "../src/gpu/bounds.ts";
import {
  collideCeilings,
  collideGuard,
  collideVerdict,
  maxContacts,
  orderCheck,
} from "../src/gpu/bounds-collide.ts";
import { cellOf, gridFor, scanPlan } from "../src/gpu/collide.ts";
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

/** The scan's workgroup width, `@workgroup_size(256)` in `kernels/collide-scan.wgsl.ts`. */
const WIDTH = 256;

/**
 * `exclusive_scan` in `collide-scan.wgsl.ts`, one workgroup's Blelloch over `tile` in place:
 * the up-sweep, the total, the cleared root, the down-sweep. Each level's invocations touch
 * disjoint indices between two barriers, so running them one after another is the same scan.
 */
function blelloch(tile) {
  let offset = 1;
  for (let d = WIDTH >> 1; d > 0; d >>= 1) {
    for (let l = 0; l < d; l += 1) {
      tile[offset * (2 * l + 2) - 1] += tile[offset * (2 * l + 1) - 1];
    }
    offset *= 2;
  }
  const total = tile[WIDTH - 1];
  tile[WIDTH - 1] = 0;
  for (let d = 1; d < WIDTH; d *= 2) {
    offset >>= 1;
    for (let l = 0; l < d; l += 1) {
      const ai = offset * (2 * l + 1) - 1;
      const bi = offset * (2 * l + 2) - 1;
      const t = tile[ai];
      tile[ai] = tile[bi];
      tile[bi] += t;
    }
  }
  return total;
}

/** The three dispatches, `scan_blocks`, `scan_sums` and `scan_add`, over `counts`. */
function threeDispatches(counts) {
  const buckets = counts.length;
  const { blocks, chunk } = scanPlan(buckets);
  const start = new Uint32Array(buckets + 1);
  const sums = new Uint32Array(blocks);
  for (let g = 0; g < blocks; g += 1) {
    const tile = new Uint32Array(WIDTH);
    tile.set(counts.subarray(g * WIDTH, Math.min((g + 1) * WIDTH, buckets)));
    sums[g] = blelloch(tile);
    start.set(tile.subarray(0, Math.min(WIDTH, buckets - g * WIDTH)), g * WIDTH);
  }
  const tile = new Uint32Array(WIDTH);
  for (let l = 0; l < WIDTH; l += 1) {
    for (let k = l * chunk; k < Math.min((l + 1) * chunk, blocks); k += 1) tile[l] += sums[k];
  }
  start[buckets] = blelloch(tile);
  for (let l = 0; l < WIDTH; l += 1) {
    let running = tile[l];
    for (let k = l * chunk; k < Math.min((l + 1) * chunk, blocks); k += 1) {
      const v = sums[k];
      sums[k] = running;
      running += v;
    }
  }
  for (let b = 0; b < buckets; b += 1) start[b] += sums[Math.floor(b / WIDTH)];
  return start;
}

test("the_collide_scan_is_stable_at_every_bucket_count", () => {
  for (const buckets of [4, 256, 65_536, 2_097_152]) {
    const { blocks, chunk } = scanPlan(buckets);
    assert.ok(blocks <= 65_535, `${buckets}: ${blocks} workgroups is over the per-dimension 65 535`);
    assert.ok(chunk * WIDTH >= blocks, `${buckets}: the second dispatch must cover every block sum`);
    // Two shapes: a deterministic spread of 0..3 per bucket, and every node in the first bucket,
    // whose carry crosses every block boundary.
    let seed = 0x9e3779b9;
    const spread = Uint32Array.from({ length: buckets }, () => {
      seed = (Math.imul(seed, 1_664_525) + 1_013_904_223) >>> 0;
      return seed >>> 30;
    });
    const crowd = new Uint32Array(buckets);
    crowd[0] = buckets >>> 1;
    for (const [shape, counts] of [["spread", spread], ["crowd", crowd]]) {
      const want = new Uint32Array(buckets + 1);
      for (let b = 0; b < buckets; b += 1) want[b + 1] = want[b] + counts[b];
      const got = threeDispatches(counts);
      const first = want.findIndex((v, b) => got[b] !== v);
      assert.equal(first, -1, `${buckets} ${shape}: start[${first}] is ${got[first]}, not ${want[first]}`);
    }
  }
});

test("the_order_check_names_a_descending_bucket", () => {
  // Three buckets over five nodes: {0, 3}, {}, {1, 2, 4}.
  const start = Uint32Array.from([0, 2, 2, 5]);
  assert.equal(orderCheck(Uint32Array.from([0, 3, 1, 2, 4]), start, 5), null, "ascending holds");
  // The collide-order fault's shape: the same members, descending.
  const reversed = orderCheck(Uint32Array.from([3, 0, 4, 2, 1]), start, 5) ?? "";
  assert.match(reversed, /^bucket 0 is not ascending/, `got ${reversed}`);
  // A member list that is ascending in every bucket but is not a permutation.
  assert.match(orderCheck(Uint32Array.from([0, 3, 0, 2, 4]), start, 5) ?? "", /node 0/);
  // Spans that do not end at n.
  assert.match(orderCheck(Uint32Array.from([0, 3, 1, 2]), Uint32Array.from([0, 2, 2, 4]), 5) ?? "", /not n/);
});

test("the_collide_guard_reads_the_measured_crowd", () => {
  // A toy crowd: four nodes, each within reach of the other three, so the largest crowd is 3.
  const posX = Float64Array.from([0, 10, 0, 10]);
  const posY = Float64Array.from([0, 0, 10, 10]);
  const grid = gridFor(posX, posY);
  assert.equal(maxContacts(posX, posY, grid), 3, "the toy crowd's largest node has 3 contacts");
  // A known reference column: every component 1, so rmsRef is 1.
  const refX = Float64Array.from([1, 1, 1, 1]);
  const refY = Float64Array.from([1, 1, 1, 1]);
  const reference = components(refX, refY);
  const rmsRef = compare({ got: reference, reference }).rmsRef;
  assert.equal(rmsRef, 1, "the toy reference's rms is 1");
  // The guard is 1e-4 + k_c * 5 * 2^-23 * P / rmsRef, with P = reach/2.
  const guard = collideGuard(3, grid.reach, rmsRef);
  const want = 1e-4 + (3 * 5 * 2 ** -23 * (grid.reach / 2)) / rmsRef;
  assert.equal(guard, want, "the guard is the measured crowd's bound");
});

test("every_collide_ceiling_sits_under_its_guard", () => {
  for (const [key, row] of Object.entries(collideCeilings())) {
    assert.match(key, /^(hardware|software):\d+:[01]$/, `${key}: a row is keyed (arm, n, state)`);
    const guard = collideGuard(row.k_c, 32, row.rmsRef);
    assert.ok(row.rmsRel <= guard, `${key}: rmsRel ${row.rmsRel} is over the guard ${guard}`);
    assert.ok(Number.isFinite(row.maxAbs) && row.maxAbs > 0, `${key}: maxAbs ${row.maxAbs}`);
  }
  // A missing row is guard only: n = 7 has no fixture, so no row.
  const clean = { n: 7, state: 0, arm: "hardware", rmsRel: 0, rmsRef: 1, k_c: 0, reach: 32, maxAbs: 1, repeatEqual: true, order: null };
  const guard = collideGuard(0, 32, 1);
  assert.deepEqual(collideVerdict({ ...clean, rmsRel: guard / 2 }).failures, []);
  const over = collideVerdict({ ...clean, rmsRel: guard * 2 });
  assert.equal(over.pass, false, "a breached guard fails the case");
  assert.match(over.failures.join(" "), /^guard \(n=7, state=0\)/, `got ${over.failures}`);
  const unordered = collideVerdict({ ...clean, order: "bucket 3 is not ascending" });
  assert.match(unordered.failures.join(" "), /^order /, `got ${unordered.failures}`);
});

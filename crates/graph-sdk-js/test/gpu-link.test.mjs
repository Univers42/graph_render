// The link pass's host half, node-only: the guard, the ceiling table, the CSR.
//
// What it pins: the guard is built from the fixture's own maximum degree, never from a
// constant (condition 8); every measured ceiling sits under the guard of the fixture it was
// measured on; and the gather's CSR files each node's edges in ascending edge index, the order
// graph-core's `row_csr` sums them in — a different order is a different sum.
//
// Each test imports its module itself, so a missing module fails that test alone.
//
// Run: node --test --experimental-strip-types crates/graph-sdk-js/test/gpu-link.test.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

/** The committed 1k fixture, as the bytes the page would fetch. */
function committed(state) {
  const bytes = readFileSync(new URL(`../../../fixtures/gpu/mesh-1k-${state}.gmfx`, import.meta.url));
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength);
}

/** A toy fixture: node 0 is a hub of degree 6, and one more edge so the hub is the maximum. */
const TOY = {
  n: 8,
  edgeLo: Uint32Array.from([0, 0, 0, 0, 0, 0, 1]),
  edgeHi: Uint32Array.from([1, 2, 3, 4, 5, 6, 7]),
};

test("the_link_guard_reads_the_measured_degree", async () => {
  const { linkGuard, measuredK } = await import("../src/gpu/bounds-link.ts");
  assert.equal(measuredK(TOY), 7, "k_measured is the maximum degree, 6, plus one");
  assert.equal(linkGuard(measuredK(TOY)), 7 * 5 * 2 ** -23);
});

test("every_link_ceiling_sits_under_its_guard", async () => {
  const { linkCeilings, linkGuard, measuredK } = await import("../src/gpu/bounds-link.ts");
  const { loadFixture } = await import("../src/gpu/fixture.ts");
  const rows = Object.entries(linkCeilings());
  assert.ok(rows.length > 0, "the link ceiling table must not be empty after a measurement run");
  const k1k = measuredK(loadFixture(committed("start")));
  for (const [key, row] of rows) {
    const guard = linkGuard(row.k);
    assert.ok(row.rmsRel <= guard, `${key}: ceiling rmsRel ${row.rmsRel} is over its guard ${guard}`);
    if (key.includes(":1000:")) {
      assert.equal(row.k, k1k, `${key}: the row's k must be the committed 1k fixture's own`);
    }
  }
});

test("the_link_csr_lists_each_node_s_edges_in_ascending_order", async () => {
  const { linkCsr } = await import("../src/gpu/link.ts");
  const { loadFixture } = await import("../src/gpu/fixture.ts");
  const toy = linkCsr(TOY.edgeLo, TOY.edgeHi, TOY.n);
  assert.deepEqual([...toy.start], [0, 6, 8, 9, 10, 11, 12, 13, 14]);
  assert.deepEqual([...toy.edges], [0, 1, 2, 3, 4, 5, 0, 6, 1, 2, 3, 4, 5, 6]);
  const fixture = loadFixture(committed("start"));
  const csr = linkCsr(fixture.edgeLo, fixture.edgeHi, fixture.n);
  assert.equal(csr.start[fixture.n], 2 * fixture.m, "every edge is filed under both ends");
  for (let node = 0; node < fixture.n; node += 1) {
    const row = csr.edges.subarray(csr.start[node], csr.start[node + 1]);
    for (let at = 0; at < row.length; at += 1) {
      const edge = row[at];
      assert.ok(at === 0 || row[at - 1] < edge, `node ${node}: row is not strictly ascending`);
      assert.ok(fixture.edgeLo[edge] === node || fixture.edgeHi[edge] === node, `node ${node}: edge ${edge} is not incident`);
    }
  }
});

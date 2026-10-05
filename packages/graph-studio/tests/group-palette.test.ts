/**
 * The document's own group column (`by: "group"` with no user groups): one palette slot per
 * group, in node order. Past the ten colours of GROUP_PALETTE the colours repeat, the slots do
 * not, so each group keeps its own legend row and its own count.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { colouringOf } from "../src/look/colourBy.ts";
import { MUTED } from "../src/look/palette.ts";
import { type GraphMeta, metaOf } from "../src/source/meta.ts";
import { node } from "./support.ts";

/** `count` nodes, one per group, the groups named `G0`…`G(count - 1)` in node order. */
function oneEach(count: number): GraphMeta {
  const names = Array.from({ length: count }, (_, i) => `G${i}`);
  const ids = names.map((_, i) => `n${i}`);
  const ends = { source: Uint32Array.of(), target: Uint32Array.of() };
  return metaOf(ids.map((id, i) => node(id, { group: names[i] ?? "" })), ids, ends);
}

test("by=group: one slot per group, so group 11 does not share group 1's colour", () => {
  const c = colouringOf({ meta: oneEach(12), by: "group", values: null, groups: [] });

  assert.deepEqual(c.colours, Uint16Array.from({ length: 12 }, (_, i) => i));
  assert.equal(c.names(10), "G10");
  assert.equal(c.names(11), "G11");
});

test("by=group: past MAX_KEYS = 512 groups the rest share one MUTED slot named (other groups)", () => {
  const c = colouringOf({ meta: oneEach(514), by: "group", values: null, groups: [] });

  const overflowSlot = c.colours[512] ?? -1;
  assert.equal(overflowSlot, 512);
  assert.equal(c.colours[513], overflowSlot);
  assert.equal(c.names(overflowSlot), "(other groups)");
  assert.deepEqual(c.palette[overflowSlot], MUTED);
});
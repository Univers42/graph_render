import assert from "node:assert/strict";
import { test } from "node:test";

import { boundsOfVisible, idsOf, withLocalHidden } from "../src/local.ts";
import { plainStyle } from "../src/style.ts";

test("idsOf lists the set nodes in ascending order", () => {
  assert.deepEqual(idsOf(Uint8Array.from([0, 1, 0, 1, 1])), [1, 3, 4]);
  assert.deepEqual(idsOf(new Uint8Array(3)), []);
});

test("withLocalHidden hides every node outside the set and keeps what the filter hid", () => {
  const base = { ...plainStyle(4), hidden: Uint8Array.from([0, 1, 0, 0]) };
  const style = withLocalHidden(base, Uint8Array.from([1, 1, 0, 1]));
  assert.deepEqual([...(style.hidden ?? [])], [0, 1, 1, 0]);
});

test("withLocalHidden without a set, or with a set of another size, is the style itself", () => {
  const base = plainStyle(3);
  assert.equal(withLocalHidden(base, null), base);
  assert.equal(withLocalHidden(base, new Uint8Array(2)), base);
});

test("boundsOfVisible is the box of the set grown by the radius of each node", () => {
  const x = Float32Array.from([0, 10, 100]);
  const y = Float32Array.from([0, 20, 100]);
  const radius = Float32Array.from([1, 2, 50]);
  const bounds = boundsOfVisible({ x, y }, Uint8Array.from([1, 1, 0]), radius);
  assert.deepEqual(bounds, { minX: -1, minY: -1, maxX: 12, maxY: 22 });
  assert.equal(boundsOfVisible({ x, y }, new Uint8Array(3), radius), null);
});

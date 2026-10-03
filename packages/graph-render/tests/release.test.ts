/** A destroyed view's bulk slot: everything it held is let go, and nothing is made for it again. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { newBulkSlot, releaseBulk } from "../src/webgl2/hook.ts";

test("a released slot holds no layer and no picture, and says why", () => {
  const slot = newBulkSlot("webgl2");
  releaseBulk(slot);
  // null, not undefined: `layerOf` makes a layer only for a slot that never had one.
  assert.equal(slot.layer, null);
  assert.equal(slot.still, null);
  assert.equal(slot.failure, "the view was destroyed");
});

test("releasing twice is harmless: a view may be destroyed by its host and by its element", () => {
  const slot = newBulkSlot("auto");
  releaseBulk(slot);
  assert.doesNotThrow(() => releaseBulk(slot));
  assert.equal(slot.layer, null);
});

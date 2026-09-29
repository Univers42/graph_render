/**
 * The navigation keys, as the studio's own actions: a key press is a dispatch, so what the
 * keyboard does is a thing the console can be asked for by name and the log records.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import type { Args, RawArgs } from "../src/actions/registry.ts";
import { type Held, ARROW_PAN, ZOOM_STEP, navKeyOf } from "../src/ui/navKeys.ts";

function check(key: string, id: string, args: Args): void {
  assert.deepEqual(navKeyOf(key), { id, args }, `key ${key}`);
}

test("f fits and 0 resets, and nothing else does either", () => {
  check("f", "view.fit", {});
  check("0", "view.reset", {});
  assert.equal(navKeyOf("F"), null, "a capital F is shift+f, and the browser finds with it");
  assert.equal(navKeyOf("9"), null);
});

test("+ and - zoom at the centre, by the step the HUD buttons use", () => {
  check("+", "view.zoom", { factor: ZOOM_STEP });
  check("=", "view.zoom", { factor: ZOOM_STEP });
  check("-", "view.zoom", { factor: 1 / ZOOM_STEP });
  check("_", "view.zoom", { factor: 1 / ZOOM_STEP });
  assert.equal(ZOOM_STEP, 2, "a press doubles the scale, or halves it");
});

test("the four arrows pan by the step, and the step is a whole number of pixels", () => {
  check("ArrowLeft", "view.pan", { dx: -ARROW_PAN, dy: 0 });
  check("ArrowRight", "view.pan", { dx: ARROW_PAN, dy: 0 });
  check("ArrowUp", "view.pan", { dx: 0, dy: -ARROW_PAN });
  check("ArrowDown", "view.pan", { dx: 0, dy: ARROW_PAN });
  assert.equal(ARROW_PAN, 50);
});

test("escape clears the selection, and a key that names nothing is null", () => {
  check("Escape", "view.clear", {});
  for (const key of ["Enter", "a", "Tab", "Home"]) {
    assert.equal(navKeyOf(key), null, key);
  }
});

test("a key held with Ctrl, Alt or the command key is the browser's", () => {
  const held = (over: Partial<Held>): Held => ({ ctrlKey: false, metaKey: false, altKey: false, ...over });
  assert.deepEqual(navKeyOf("f", held({ ctrlKey: true })), null);
  assert.deepEqual(navKeyOf("ArrowLeft", held({ metaKey: true })), null);
  assert.deepEqual(navKeyOf("0", held({ altKey: true })), null);
});

test("what a key asks for is what dispatch takes, unchanged", () => {
  const nav = navKeyOf("ArrowRight");
  assert.ok(nav !== null);
  const raw: RawArgs = { ...nav.args };
  assert.deepEqual(raw, { dx: 50, dy: 0 });
});

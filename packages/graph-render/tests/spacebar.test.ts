/** The space bar, read without a browser: it is one flag, and it is the drag modifier. */
import assert from "node:assert/strict";
import { test } from "node:test";

import { forget, isDown, press, release } from "../src/spacebar.ts";

function key(name: string, code = name, repeat = false): { key: string; code: string; repeat: boolean } {
  return { key: name, code, repeat };
}

test("the space bar goes down and comes up", () => {
  forget();
  assert.equal(isDown(), false);
  press(key(" ", "Space"));
  assert.equal(isDown(), true);
  release(key(" ", "Space"));
  assert.equal(isDown(), false);
});

test("a key that is not the space bar changes nothing", () => {
  forget();
  press(key("a"));
  assert.equal(isDown(), false);
  release(key("a"));
  assert.equal(isDown(), false);
});

test("a held space bar repeats as the same answer, and blur lets go", () => {
  forget();
  press(key(" ", "Space", true));
  assert.equal(isDown(), true);
  forget();
  assert.equal(isDown(), false, "losing the focus with the bar down must not keep it down");
});

test("the space bar is found by its code when the layout prints something else", () => {
  forget();
  press(key("␣", "Space"));
  assert.equal(isDown(), true);
  release(key("␣", "Space"));
  assert.equal(isDown(), false);
});

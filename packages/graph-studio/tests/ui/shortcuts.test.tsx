// Which key does what: the mapping is a function of the key and of who has the keyboard.
import assert from "node:assert/strict";
import { test } from "node:test";

import { shortcutOf } from "../../src/ui/useShortcuts.ts";

test("a key that means nothing here means nothing", () => {
  assert.equal(shortcutOf("a", false), null);
  assert.equal(shortcutOf("Enter", false), null);
  assert.equal(shortcutOf("F", false), null, "a key that is not a word is not one of these");
});

test("a backquote opens and closes the console, typing or not", () => {
  assert.equal(shortcutOf("Backquote", false), "console");
  assert.equal(shortcutOf("Backquote", true), "console");
});

test("a slash searches and an f fits, unless a field holds the keyboard", () => {
  assert.equal(shortcutOf("/", false), "search");
  assert.equal(shortcutOf("f", false), "fit");
  assert.equal(shortcutOf("/", true), null);
  assert.equal(shortcutOf("f", true), null);
});

test("escape always means escape: it is the way out of whatever is open", () => {
  assert.equal(shortcutOf("Escape", false), "escape");
  assert.equal(shortcutOf("Escape", true), "escape");
});

// Which key does what: the mapping is a function of the key and of who has the keyboard.
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Pressed, shortcutOf } from "../../src/ui/useShortcuts.ts";

function pressed(key: string, over: Partial<Pressed> = {}): Pressed {
  return { key, code: "", ctrlKey: false, metaKey: false, altKey: false, ...over };
}

test("a key that means nothing here means nothing", () => {
  assert.equal(shortcutOf(pressed("a"), false), null);
  assert.equal(shortcutOf(pressed("Enter"), false), null);
  assert.equal(shortcutOf(pressed("F"), false), null, "a capital is a key held with Shift");
});

test("a backquote opens and closes the console, typing or not", () => {
  assert.equal(shortcutOf(pressed("`"), false), "console");
  assert.equal(shortcutOf(pressed("`"), true), "console");
});

test("the key left of 1 is the console key on a keyboard that prints no backquote there", () => {
  assert.equal(shortcutOf(pressed("º", { code: "Backquote" }), false), "console");
  assert.equal(shortcutOf(pressed("²", { code: "Backquote" }), true), "console");
});

test("a slash searches and an f fits, unless a field holds the keyboard", () => {
  assert.equal(shortcutOf(pressed("/"), false), "search");
  assert.equal(shortcutOf(pressed("f"), false), "fit");
  assert.equal(shortcutOf(pressed("/"), true), null);
  assert.equal(shortcutOf(pressed("f"), true), null);
});

test("a key held with Ctrl, Alt or the command key belongs to the browser", () => {
  assert.equal(shortcutOf(pressed("f", { ctrlKey: true }), false), null, "Ctrl+F finds in the page");
  assert.equal(shortcutOf(pressed("f", { metaKey: true }), false), null);
  assert.equal(shortcutOf(pressed("/", { altKey: true }), false), null);
  assert.equal(shortcutOf(pressed("`", { ctrlKey: true }), false), null);
});

test("escape always means escape: it is the way out of whatever is open", () => {
  assert.equal(shortcutOf(pressed("Escape"), false), "escape");
  assert.equal(shortcutOf(pressed("Escape"), true), "escape");
});

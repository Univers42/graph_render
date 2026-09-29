// Which key does what: the mapping is a function of the key and of who has the keyboard.
import assert from "node:assert/strict";
import { test } from "node:test";

import { type Pressed, chromeOf, navigationOf } from "../../src/ui/useShortcuts.ts";

function pressed(key: string, over: Partial<Pressed> = {}): Pressed {
  return { key, code: "", ctrlKey: false, metaKey: false, altKey: false, ...over };
}

test("a key that means nothing here means nothing", () => {
  assert.equal(chromeOf(pressed("a"), false), null);
  assert.equal(chromeOf(pressed("Enter"), false), null);
  assert.equal(navigationOf(pressed("Enter"), false), null);
});

test("a backquote opens and closes the console, typing or not", () => {
  assert.equal(chromeOf(pressed("`"), false), "console");
  assert.equal(chromeOf(pressed("`"), true), "console");
});

test("the key left of 1 is the console key on a keyboard that prints no backquote there", () => {
  assert.equal(chromeOf(pressed("º", { code: "Backquote" }), false), "console");
  assert.equal(chromeOf(pressed("²", { code: "Backquote" }), true), "console");
});

test("a slash searches unless a field holds the keyboard", () => {
  assert.equal(chromeOf(pressed("/"), false), "search");
  assert.equal(chromeOf(pressed("/"), true), null);
});

test("escape is the way out of whatever is open, typing or not", () => {
  assert.equal(chromeOf(pressed("Escape"), false), "escape");
  assert.equal(chromeOf(pressed("Escape"), true), "escape");
  assert.deepEqual(navigationOf(pressed("Escape"), false), { id: "view.clear", args: {} });
  assert.equal(navigationOf(pressed("Escape"), true), null, "and it is the chrome's, not the camera's");
});

test("the camera keys are actions, and the chrome keeps none of them", () => {
  assert.equal(chromeOf(pressed("f"), false), null, "f moved to the action registry");
  assert.deepEqual(navigationOf(pressed("f"), false), { id: "view.fit", args: {} });
  assert.deepEqual(navigationOf(pressed("0"), false), { id: "view.reset", args: {} });
  assert.deepEqual(navigationOf(pressed("+"), false), { id: "view.zoom", args: { factor: 2 } });
  assert.deepEqual(navigationOf(pressed("-"), false), { id: "view.zoom", args: { factor: 0.5 } });
  assert.deepEqual(navigationOf(pressed("ArrowRight"), false), { id: "view.pan", args: { dx: 50, dy: 0 } });
});

test("a field that holds the keyboard takes the camera keys, and keeps Escape for the chrome", () => {
  for (const key of ["f", "0", "+", "-", "ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"]) {
    assert.equal(navigationOf(pressed(key), true), null, key);
  }
});

test("a key held with Ctrl, Alt or the command key belongs to the browser", () => {
  assert.equal(chromeOf(pressed("f", { ctrlKey: true }), false), null, "Ctrl+F finds in the page");
  assert.equal(navigationOf(pressed("f", { ctrlKey: true }), false), null);
  assert.equal(navigationOf(pressed("f", { metaKey: true }), false), null);
  assert.equal(navigationOf(pressed("ArrowLeft", { altKey: true }), false), null);
  assert.equal(navigationOf(pressed("0", { metaKey: true }), false), null);
  assert.equal(chromeOf(pressed("/", { altKey: true }), false), null);
  assert.equal(chromeOf(pressed("`", { ctrlKey: true }), false), null);
});

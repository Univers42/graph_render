// The node menu: what it offers, how it moves, and what it writes.
import assert from "node:assert/strict";
import { test } from "node:test";
import { createElement } from "react";

import { NodeMenu } from "../../src/ui/NodeMenu.tsx";
import { entriesFor, nextEntry } from "../../src/ui/nodeMenu.ts";
import { DRAWN, fakeView, markup, studioWith } from "./desk.ts";

function menu(pinned: readonly number[], at: { node: number } | null = { node: 0 }): string {
  const { studio } = studioWith(DRAWN);
  const menuAt = at === null ? null : { node: at.node, at: { x: 10, y: 20 } };
  return markup(createElement(NodeMenu, { studio, state: DRAWN, view: fakeView(pinned), menu: menuAt, onClose: () => undefined }));
}

test("the menu offers focus, pin, hide and copy id, in that order", () => {
  assert.deepEqual(entriesFor(false).map((entry) => entry.label), ["Focus", "Pin", "Hide", "Copy id"]);
  assert.deepEqual(entriesFor(true).map((entry) => entry.label), ["Focus", "Unpin", "Hide", "Copy id"]);
});

test("arrows wrap round, Home and End go to the ends, other keys stay", () => {
  assert.equal(nextEntry(3, "ArrowDown", 4), 0);
  assert.equal(nextEntry(0, "ArrowUp", 4), 3);
  assert.equal(nextEntry(2, "Home", 4), 0);
  assert.equal(nextEntry(1, "End", 4), 3);
  assert.equal(nextEntry(1, "a", 4), 1);
});

test("nothing is drawn while the menu is closed", () => {
  assert.equal(menu([], null), "");
});

test("the open menu is a menu of items, and a pinned node offers Unpin", () => {
  const html = menu([0]);
  assert.match(html, /role="menu"/);
  assert.equal((html.match(/role="menuitem"/g) ?? []).length, 4);
  assert.ok(html.includes(">Unpin<") && !html.includes(">Pin<"));
  assert.ok(menu([1]).includes(">Pin<"));
});

test("copying an id puts it in the studio's clipboard state", () => {
  const { studio } = studioWith(DRAWN);
  studio.copy("b");
  assert.equal(studio.store.get().clipboard, "b");
});

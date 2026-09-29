// The key overlay lists every binding the two key tables hold, no more and no less.
import assert from "node:assert/strict";
import { test } from "node:test";

import { KeyOverlay } from "../../src/ui/KeyOverlay.tsx";
import { CHROME_KEYS } from "../../src/ui/keymap.ts";
import { KEYS } from "../../src/ui/navKeys.ts";
import { chromeOf } from "../../src/ui/useShortcuts.ts";
import { markup } from "./desk.ts";

function rowsOf(html: string): string[] {
  return [...html.matchAll(/<tr data-key="([^"]*)" data-action="([^"]*)"/g)].map((m) => `${m[1]}=${m[2]}`);
}

test("a closed overlay draws nothing", () => {
  assert.equal(markup(<KeyOverlay open={false} onClose={() => undefined} />), "");
});

test("an open overlay has one row per camera key and per chrome key, in table order", () => {
  const html = markup(<KeyOverlay open onClose={() => undefined} />);
  const want = [
    ...Object.entries(KEYS).map(([key, nav]) => `${key}=${nav.id}`),
    ...CHROME_KEYS.map((row) => `${row.key}=${row.id}`),
  ];
  assert.deepEqual(rowsOf(html), want);
});

test("a row shows the action's title, and the dialog has a close button", () => {
  const html = markup(<KeyOverlay open onClose={() => undefined} />);
  assert.match(html, /role="dialog"/);
  assert.match(html, /aria-label="Close"/);
  assert.match(html, /<td>view\.fit<\/td><td>[^<]+<\/td>/);
});

test("the question mark is the chrome's help key, unless a field holds the keyboard", () => {
  const key = { key: "?", code: "", ctrlKey: false, metaKey: false, altKey: false };
  assert.equal(chromeOf(key, false), "help");
  assert.equal(chromeOf(key, true), null);
});

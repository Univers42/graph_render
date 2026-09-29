// Gate row `settings-frozen`: the settings are one frozen document, and a change is a
// new document.
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DEFAULT_SETTINGS, SettingsRefusal, readSettings, sameSettings, withAppearance, withFilter, withSettings,
} from "../src/state/settings.ts";

function everyObjectIn(value: unknown, found: object[] = []): object[] {
  if (typeof value !== "object" || value === null) return found;
  found.push(value);
  for (const member of Object.values(value)) everyObjectIn(member, found);
  return found;
}

test("the defaults are frozen all the way down", () => {
  const objects = everyObjectIn(DEFAULT_SETTINGS);
  assert.ok(objects.length >= 5);
  for (const object of objects) assert.ok(Object.isFrozen(object), JSON.stringify(object));
});

test("writing to the settings throws instead of changing them", () => {
  assert.throws(() => Object.assign(DEFAULT_SETTINGS, { layout: "layout.grid" }), TypeError);
  assert.throws(() => Object.assign(DEFAULT_SETTINGS.appearance, { theme: "light" }), TypeError);
  assert.equal(DEFAULT_SETTINGS.appearance.theme, "dark");
});

test("a change is a new frozen document that shares what it did not touch", () => {
  const next = withAppearance(DEFAULT_SETTINGS, { theme: "light" });
  assert.notEqual(next, DEFAULT_SETTINGS);
  assert.equal(next.appearance.theme, "light");
  assert.equal(DEFAULT_SETTINGS.appearance.theme, "dark");
  assert.equal(next.filter, DEFAULT_SETTINGS.filter);
  for (const object of everyObjectIn(next)) assert.ok(Object.isFrozen(object));
});

test("a filter change freezes the list it was given a copy of", () => {
  const hidden = ["Alpha"];
  const next = withFilter(DEFAULT_SETTINGS, { hiddenGroups: hidden });
  hidden.push("Beta");
  assert.deepEqual(next.filter.hiddenGroups, ["Alpha"]);
  assert.ok(Object.isFrozen(next.filter.hiddenGroups));
});

test("settings survive JSON and compare equal afterwards", () => {
  const changed = withSettings(withFilter(DEFAULT_SETTINGS, { text: "graph", minDegree: 2 }), {
    layout: "layout.grid", edges: "post.style.bezier", source: { kind: "fixture", path: "dag/chain.json" },
  });
  const back = readSettings(JSON.parse(JSON.stringify(changed)));
  assert.deepEqual(back, changed);
  assert.ok(sameSettings(back, changed));
  assert.ok(!sameSettings(back, DEFAULT_SETTINGS));
  for (const object of everyObjectIn(back)) assert.ok(Object.isFrozen(object));
});

const REFUSED: readonly (readonly [string, unknown, RegExp])[] = [
  ["not an object", 7, /settings: not an object/],
  ["a missing member", { ...DEFAULT_SETTINGS, layout: undefined }, /settings.layout: not a string/],
  ["an unknown member", { ...DEFAULT_SETTINGS, physics: true }, /settings.physics: not a member/],
  ["an unknown source", { ...DEFAULT_SETTINGS, source: { kind: "server" } }, /settings.source.kind/],
  ["a node count that is not a whole number", { ...DEFAULT_SETTINGS, source: { kind: "synthetic", seed: 1, nodes: 1.5, degree: 2, shape: "vault" } }, /settings.source.nodes/],
  ["a theme that does not exist", { ...DEFAULT_SETTINGS, appearance: { ...DEFAULT_SETTINGS.appearance, theme: "sepia" } }, /settings.appearance.theme/],
  ["a node scale out of range", { ...DEFAULT_SETTINGS, appearance: { ...DEFAULT_SETTINGS.appearance, nodeScale: 40 } }, /settings.appearance.nodeScale/],
  ["a hidden group that is not a string", { ...DEFAULT_SETTINGS, filter: { ...DEFAULT_SETTINGS.filter, hiddenGroups: [1] } }, /settings.filter.hiddenGroups/],
];

for (const [name, value, message] of REFUSED) {
  test(`${name} is refused`, () => {
    assert.throws(() => readSettings(value), (error: unknown) => error instanceof SettingsRefusal && message.test(error.message));
  });
}

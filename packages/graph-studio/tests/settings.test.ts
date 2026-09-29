// Gate row `settings-frozen`: the settings are one frozen document, and a change is a
// new document.
import assert from "node:assert/strict";
import { test } from "node:test";

import {
  COLOUR_BY, DEFAULT_SETTINGS, type Group, SettingsRefusal, readSettings, sameSettings, withAppearance, withFilter,
  withGroups, withSettings,
} from "../src/state/settings.ts";

const NOTHING_FILTER = {
  query: "", text: "", hiddenKinds: [], hiddenGroups: [], orphans: false, existingOnly: false,
  minDegree: 0, relayout: false,
};

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

test("the default document is the one the studio opens with", () => {
  assert.deepEqual(DEFAULT_SETTINGS.filter, NOTHING_FILTER);
  assert.deepEqual(DEFAULT_SETTINGS.groups, []);
  assert.deepEqual([...COLOUR_BY], ["group", "kind", "tag", "db", "analysis", "none"]);
});

test("a filter cannot be changed after it is written", () => {
  const next = withFilter(DEFAULT_SETTINGS, { orphans: true, query: "kind:tag", hiddenKinds: ["tag"] });
  assert.throws(() => Object.assign(next.filter, { orphans: false }), TypeError);
  assert.throws(() => Object.assign(next.filter.hiddenKinds, { 0: "note" }), TypeError);
  assert.equal(next.filter.orphans, true);
  assert.deepEqual(next.filter.hiddenKinds, ["tag"]);
});

/** A frozen array, written to: what a caller would do, and what a frozen list refuses. */
function at(groups: readonly Group[], i: number): Group {
  const found = groups[i];
  if (found === undefined) throw new Error(`no group at ${i}`);
  return found;
}

test("the group list is frozen, and a change is a new list", () => {
  const groups: readonly Group[] = [{ name: "Alpha", query: "kind:database", colour: "#7c9cf5" }];
  const next = withGroups(DEFAULT_SETTINGS, groups);
  assert.throws(() => Object.assign(at(next.groups, 0), { name: "Beta" }), TypeError);
  assert.throws(() => Object.assign(next.groups, { 0: { name: "Beta", query: "", colour: "" } }), TypeError);
  assert.equal(next.groups.length, 1);
  assert.equal(at(next.groups, 0).name, "Alpha");
  assert.deepEqual(DEFAULT_SETTINGS.groups, []);
});

test("a full document reads back, and one missing a member is refused", () => {
  const full = withGroups(withFilter(DEFAULT_SETTINGS, { orphans: true, existingOnly: true, relayout: true }), [
    { name: "Alpha", query: "kind:database", colour: "#7c9cf5" },
    { name: "Beta", query: "kind:note OR tag:slow", colour: "#f2a65a" },
  ]);
  const back = readSettings(JSON.parse(JSON.stringify(full)));
  assert.deepEqual(back, full);
  assert.ok(sameSettings(back, full));
  const without = { ...full, groups: undefined };
  assert.throws(() => readSettings(without), /settings\.groups: not a list/);
});

const BAD_FILTERS: readonly (readonly [string, unknown, RegExp])[] = [
  ["orphans that is not a boolean", { ...NOTHING_FILTER, orphans: "yes" }, /settings\.filter\.orphans/],
  ["existingOnly that is not a boolean", { ...NOTHING_FILTER, existingOnly: 1 }, /settings\.filter\.existingOnly/],
  ["relayout that is not a boolean", { ...NOTHING_FILTER, relayout: null }, /settings\.filter\.relayout/],
  ["a hidden kind that is not a string", { ...NOTHING_FILTER, hiddenKinds: [2] }, /settings\.filter\.hiddenKinds\[0\]/],
  ["a query that is not a string", { ...NOTHING_FILTER, query: null }, /settings\.filter\.query/],
  ["a member the filter does not have", { ...NOTHING_FILTER, colour: "red" }, /settings\.filter\.colour: not a member/],
];

for (const [name, filter, message] of BAD_FILTERS) {
  test(`${name} is refused`, () => {
    assert.throws(
      () => readSettings({ ...DEFAULT_SETTINGS, filter }),
      (error: unknown) => error instanceof SettingsRefusal && message.test(error.message),
    );
  });
}

const BAD_GROUPS: readonly (readonly [string, unknown, RegExp])[] = [
  ["a group that is not a list", {}, /settings\.groups: not a list/],
  ["a group with no colour", [{ name: "Alpha", query: "" }], /settings\.groups\[0\]\.colour/],
  ["a group that is not an object", ["Alpha"], /settings\.groups\[0\]: not an object/],
  ["a group with an unknown member", [{ name: "Alpha", query: "", colour: "", weight: 1 }], /not a member/],
];

for (const [name, groups, message] of BAD_GROUPS) {
  test(`${name} is refused`, () => {
    assert.throws(
      () => readSettings({ ...DEFAULT_SETTINGS, groups }),
      (error: unknown) => error instanceof SettingsRefusal && message.test(error.message),
    );
  });
}

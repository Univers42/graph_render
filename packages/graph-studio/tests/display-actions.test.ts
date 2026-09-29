// Each display control is an action, so the console and the panel land on the same settings.
import assert from "node:assert/strict";
import { test } from "node:test";

import { THEME_NAMES, themeNamed } from "../../graph-render/src/look/themes.ts";
import { DEFAULT_SETTINGS } from "../src/state/settings.ts";
import { desk, refusingClient } from "./desk.ts";

const CASES: readonly [string, Record<string, unknown>, Record<string, unknown>][] = [
  ["arrows", { on: true }, { arrows: true }],
  ["fade", { value: -3 }, { textFade: -3 }],
  ["scale", { factor: 5 }, { nodeScale: 5 }],
  ["thickness", { factor: 0.1 }, { linkThickness: 0.1 }],
  ["edgestyle", { style: "curve" }, { edgeStyle: "curve" }],
  ["glow", { on: true }, { glow: true }],
  ["glowstrength", { value: 2 }, { glowStrength: 2 }],
  ["theme", { name: "obsidian-light" }, { theme: "obsidian-light" }],
  ["theme", { name: "blueprint" }, { theme: "blueprint" }],
  ["background", { mode: "aurora" }, { background: "aurora" }],
  ["background", { mode: "flat" }, { background: "flat" }],
  ["minpx", { px: 2 }, { minRadius: 2 }],
  ["maxpx", { px: 8 }, { maxRadius: 8 }],
];

for (const [id, raw, patch] of CASES) {
  test(`${id} ${JSON.stringify(raw)} sets ${JSON.stringify(patch)} and nothing else`, async () => {
    const { studio } = desk(refusingClient());
    assert.equal((await studio.dispatch(id, raw)).ok, true);
    assert.deepEqual(studio.store.get().settings.appearance, { ...DEFAULT_SETTINGS.appearance, ...patch });
  });
}

test("every theme name reaches the view as its own theme, with no motor call", async () => {
  const { studio, seen } = desk(refusingClient());
  for (const name of THEME_NAMES) await studio.dispatch("theme", { name });
  assert.deepEqual(seen.themes, THEME_NAMES.map(themeNamed));
});

test("a value outside the range is refused and the settings stay put", async () => {
  const { studio } = desk(refusingClient());
  for (const [id, raw] of [["fade", { value: 3.5 }], ["thickness", { factor: 6 }], ["scale", { factor: 0.1 }]] as const) {
    assert.equal((await studio.dispatch(id, raw)).ok, false, id);
  }
  assert.deepEqual(studio.store.get().settings, DEFAULT_SETTINGS);
});

test("animate and its stop are actions; with no graph nothing is hidden and the settings stay put", async () => {
  const { studio } = desk(refusingClient());
  assert.equal((await studio.dispatch("animate", { ms: 200 })).message, "0 nodes shown");
  assert.equal((await studio.dispatch("animatestop")).message, "nothing is being revealed");
  assert.equal((await studio.dispatch("animate", { ms: 99999 })).ok, false);
  assert.deepEqual(studio.store.get().settings, DEFAULT_SETTINGS);
  assert.equal(studio.store.get().reveal, null);
});

test("the animate action's default duration is 10000 ms", () => {
  const { studio } = desk(refusingClient());
  const animate = studio.registry.actions.find((action) => action.alias === "animate");
  const param = animate?.params[0];
  assert.equal(param?.value(studio.store.get()), 10_000);
});

test("node size bounds: min and max px apply in turn, and an inverted pair is refused with a reason", async () => {
  const { studio } = desk(refusingClient());
  assert.equal((await studio.dispatch("minpx", { px: 8 })).ok, true);
  assert.equal((await studio.dispatch("maxpx", { px: 8 })).ok, true);
  const before = studio.store.get().settings;
  const refused = await studio.dispatch("maxpx", { px: 2 });
  assert.equal(refused.ok, false);
  assert.match(refused.message, /below the minimum/);
  assert.equal((await studio.dispatch("minpx", { px: 9 })).ok, false);
  assert.deepEqual(studio.store.get().settings, before);
});

test("the background mode is one of theme, flat, aurora; anything else is refused", async () => {
  const { studio } = desk(refusingClient());
  assert.equal(studio.store.get().settings.appearance.background, "theme");
  assert.equal((await studio.dispatch("background", { mode: "plaid" })).ok, false);
  assert.equal(studio.store.get().settings.appearance.background, "theme");
});

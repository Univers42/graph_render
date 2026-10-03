/**
 * The force knobs as a saved member of the settings: a document from before they were saved
 * loads the motor's defaults, a partial one fills in the rest, and a wrong value is refused
 * with the member named, like every other member.
 */
import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_KNOBS, type ForceKnobs } from "../src/motor/live.ts";
import { readForces } from "../src/state/forces.ts";
import { type SettingsStorage, recall, remember } from "../src/state/persist.ts";
import { exportSettings, importSettings } from "../src/state/portable.ts";
import { DEFAULT_SETTINGS, SettingsRefusal, withSettings } from "../src/state/settings.ts";
import { desk, refusingClient } from "./desk.ts";
import { DRAWN } from "./drawn.ts";

const SPREAD: ForceKnobs = { ...DEFAULT_KNOBS, collideRadius: 9, charge: -270, theta: 1.2 };
const SPREAD_SETTINGS = withSettings(DEFAULT_SETTINGS, { forces: SPREAD });

/** The exported document with its `forces` member taken out: what a studio before this one wrote. */
function olderDocument(): string {
  return JSON.stringify(Object.fromEntries(Object.entries(SPREAD_SETTINGS).filter(([member]) => member !== "forces")));
}

function refused(value: unknown, member: RegExp): void {
  assert.throws(() => readForces(value), (error: unknown) => error instanceof SettingsRefusal && member.test(error.message));
}

test("a document saved before the knobs were loads the motor's defaults", () => {
  assert.deepEqual(importSettings(olderDocument()).forces, DEFAULT_KNOBS);
  assert.deepEqual(readForces(undefined), DEFAULT_KNOBS);
});

test("a document saved before a knob existed fills that knob in with its default", () => {
  const four = { gravity: 0.3, charge: -400, linkStrengthScale: 1, linkDistance: 80 };
  assert.deepEqual(readForces(four), { ...DEFAULT_KNOBS, ...four });
});

test("a knob outside the panel's range, or not a number, is refused with its name", () => {
  refused({ collideRadius: 401 }, /collideRadius/);
  refused({ velocityDecay: 0 }, /velocityDecay/);
  refused({ theta: "fast" }, /theta/);
  refused({ distanceMax: Number.NaN }, /distanceMax/);
});

test("a member that is not a knob is refused, not dropped", () => {
  refused({ strength: 3 }, /strength/);
  refused([1, 2], /forces/);
});

test("export then import keeps every knob, in one fixed order", () => {
  assert.deepEqual(importSettings(exportSettings(SPREAD_SETTINGS)).forces, SPREAD);
  const reordered = { ...Object.fromEntries(Object.entries(SPREAD).reverse()) };
  const text = exportSettings(withSettings(DEFAULT_SETTINGS, { forces: readForces(reordered) }));
  assert.equal(text, exportSettings(SPREAD_SETTINGS));
});

test("the knobs are remembered per source with the rest of the settings", () => {
  const items = new Map<string, string>();
  const storage: SettingsStorage = { getItem: (key) => items.get(key) ?? null, setItem: (key, value) => void items.set(key, value) };
  remember(storage, SPREAD_SETTINGS);
  assert.deepEqual(recall(storage, DEFAULT_SETTINGS.source)?.forces, SPREAD);
});

test("settings.import writes the document's knobs into the studio, and an older one the defaults", async () => {
  const made = desk(refusingClient());
  made.studio.store.set({ ...DRAWN, settings: SPREAD_SETTINGS });
  const older = await made.studio.dispatch("settings.import", { text: olderDocument() });
  assert.equal(older.ok, true, older.message);
  assert.deepEqual(made.studio.store.get().settings.forces, DEFAULT_KNOBS);
  const entry = await made.studio.dispatch("settings.import", { text: exportSettings(SPREAD_SETTINGS) });
  assert.equal(entry.ok, true, entry.message);
  assert.deepEqual(made.studio.store.get().settings.forces, SPREAD);
});

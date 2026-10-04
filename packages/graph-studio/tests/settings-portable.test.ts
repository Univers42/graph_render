// Settings leave the studio as JSON and come back byte for byte; a section resets alone.
import assert from "node:assert/strict";
import { test } from "node:test";

import { exportSettings, importSettings } from "../src/state/portable.ts";
import { withAppearance, withFilter } from "../src/state/settings.ts";
import { desk, refusingClient } from "./desk.ts";
import { DRAWN } from "./drawn.ts";

/** The drawn document's own settings, so a panel reset on them plans no relayout. */
const BASE = DRAWN.settings;
const CHANGED = withFilter(withAppearance(BASE, { theme: "light", nodeScale: 2 }), { minDegree: 3 });

function changed(): ReturnType<typeof desk> {
  const made = desk(refusingClient());
  made.studio.store.set({ ...DRAWN, settings: CHANGED });
  return made;
}

function textsOf(made: ReturnType<typeof desk>): Promise<string[]> {
  return Promise.all(made.saved.map((file) => file.data.text()));
}

test("export then import is the identity on the document", () => {
  assert.deepEqual(importSettings(exportSettings(CHANGED)), CHANGED);
  assert.equal(exportSettings(importSettings(exportSettings(CHANGED))), exportSettings(CHANGED));
});

test("importing text that is not settings is refused with the member named", () => {
  assert.throws(() => importSettings("{\"layout\": 3}"), /settings/);
  assert.throws(() => importSettings("nope"), /JSON/);
});

test("settings.export saves settings.json holding the exported document", async () => {
  const made = changed();
  const entry = await made.studio.dispatch("settings.export");
  assert.equal(entry.ok, true);
  assert.deepEqual(made.saved.map((file) => file.name), ["settings.json"]);
  assert.deepEqual(await textsOf(made), [exportSettings(CHANGED)]);
});

test("export, reset the panels, import: the export is byte-identical", async () => {
  const made = changed();
  await made.studio.dispatch("settings.export");
  const [before = ""] = await textsOf(made);
  await made.studio.dispatch("settings.reset.appearance");
  await made.studio.dispatch("settings.reset.filter");
  assert.deepEqual(made.studio.store.get().settings, BASE);
  const entry = await made.studio.dispatch("settings.import", { text: before });
  assert.equal(entry.ok, true, entry.message);
  await made.studio.dispatch("settings.export");
  assert.deepEqual(await textsOf(made), [before, before]);
});

test("resetting one panel restores only that panel's keys", async () => {
  const made = changed();
  await made.studio.dispatch("settings.reset.filter");
  const { settings } = made.studio.store.get();
  assert.deepEqual(settings.filter, BASE.filter);
  assert.deepEqual(settings.appearance, CHANGED.appearance);
  await made.studio.dispatch("settings.reset.appearance");
  assert.deepEqual(made.studio.store.get().settings.appearance, BASE.appearance);
});

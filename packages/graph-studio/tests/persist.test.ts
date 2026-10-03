// Settings persist per source in localStorage; a store that throws leaves the defaults.
import assert from "node:assert/strict";
import { test } from "node:test";

import { initialState } from "../src/state/model.ts";
import { createStore } from "../src/state/store.ts";
import { type SettingsStorage, keepSettings, openingSettings, recall, remember } from "../src/state/persist.ts";
import { DEFAULT_SETTINGS, type Settings, withAppearance, withFilter, withParams, withSettings } from "../src/state/settings.ts";

function memory(): SettingsStorage & { readonly items: Map<string, string> } {
  const items = new Map<string, string>();
  return { items, getItem: (key) => items.get(key) ?? null, setItem: (key, value) => void items.set(key, value) };
}

const throwing: SettingsStorage = {
  getItem: () => { throw new Error("blocked"); },
  setItem: () => { throw new Error("blocked"); },
};

const other = { kind: "fixture", path: "b.json" } as const;

function changed(): Settings {
  return withFilter(withAppearance(DEFAULT_SETTINGS, { theme: "light", nodeScale: 2 }), { minDegree: 3 });
}

test("what was remembered for a source is recalled for it, as an equal document", () => {
  const storage = memory();
  remember(storage, changed());
  assert.deepEqual(recall(storage, DEFAULT_SETTINGS.source), changed());
});

test("a second source keeps its own settings", () => {
  const storage = memory();
  remember(storage, changed());
  const second = withAppearance(withSettings(DEFAULT_SETTINGS, { source: other }), { labels: "none" });
  remember(storage, second);
  assert.deepEqual(recall(storage, DEFAULT_SETTINGS.source), changed());
  assert.deepEqual(recall(storage, other), second);
});

test("opening settings are those of the source last remembered, or the defaults", () => {
  const storage = memory();
  assert.deepEqual(openingSettings(storage), DEFAULT_SETTINGS);
  const second = withSettings(DEFAULT_SETTINGS, { source: other, layout: "layout.grid" });
  remember(storage, changed());
  remember(storage, second);
  assert.deepEqual(openingSettings(storage), second);
});

test("a storage that throws yields the defaults and does not throw", () => {
  assert.doesNotThrow(() => remember(throwing, changed()));
  assert.equal(recall(throwing, DEFAULT_SETTINGS.source), null);
  assert.deepEqual(openingSettings(throwing), DEFAULT_SETTINGS);
  assert.deepEqual(openingSettings(null), DEFAULT_SETTINGS);
});

test("a stored value that is not settings is ignored", () => {
  const storage = memory();
  remember(storage, changed());
  for (const key of storage.items.keys()) storage.items.set(key, "{\"layout\": 4}");
  assert.equal(recall(storage, DEFAULT_SETTINGS.source), null);
  assert.deepEqual(openingSettings(storage), DEFAULT_SETTINGS);
  for (const key of storage.items.keys()) storage.items.set(key, "not json");
  assert.deepEqual(openingSettings(storage), DEFAULT_SETTINGS);
});

test("keepSettings writes when the settings change and not when something else does", () => {
  const storage = memory();
  let writes = 0;
  const counting: SettingsStorage = { getItem: storage.getItem, setItem: (key, value) => { writes += 1; storage.setItem(key, value); } };
  const store = createStore({ ...initialState(), settings: DEFAULT_SETTINGS });
  const stop = keepSettings(store, counting);
  store.update((state) => ({ ...state, busy: [{ seq: 1, command: "x" }] }));
  assert.equal(writes, 0);
  store.update((state) => ({ ...state, settings: changed() }));
  assert.equal(writes, 2, "the document and the last-source pointer");
  assert.deepEqual(recall(storage, DEFAULT_SETTINGS.source), changed());
  stop();
  store.update((state) => ({ ...state, settings: DEFAULT_SETTINGS }));
  assert.equal(writes, 2);
});

test("what each layout is run at is remembered with the source, and comes back with it", () => {
  const storage = memory();
  const held = withParams(changed(), "layout.force.graphopt", { niter: 250 });
  remember(storage, held);
  assert.deepEqual(recall(storage, DEFAULT_SETTINGS.source), held);
  assert.deepEqual(recall(storage, DEFAULT_SETTINGS.source)?.params, { "layout.force.graphopt": { niter: 250 } });
});

test("a store that keeps the settings keeps the values with them", () => {
  const storage = memory();
  const store = createStore(initialState(DEFAULT_SETTINGS));
  const stop = keepSettings(store, storage);
  const held = withParams(DEFAULT_SETTINGS, "layout.grid", { spacing: 24 });
  store.update((state) => ({ ...state, settings: held }));
  assert.deepEqual(recall(storage, DEFAULT_SETTINGS.source)?.params, { "layout.grid": { spacing: 24 } });
  stop();
});

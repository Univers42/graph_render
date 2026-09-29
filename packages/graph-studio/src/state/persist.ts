/**
 * Settings that outlive a page load, one document per source, in a storage the caller gives.
 * Every access is in try/catch: a private window or blocked site data leaves the defaults.
 *
 * Ponytail: a document source is keyed by its name, length and a 32-bit hash of its text,
 * so two different files of one name and length that collide share settings (about 1 in 4e9
 * per pair). The storage's quota is not checked; a full store drops the write silently.
 */
import type { StudioState } from "./model.ts";
import type { Store } from "./store.ts";
import { type Settings, type Source, DEFAULT_SETTINGS, readSettings } from "./settings.ts";

export type SettingsStorage = Pick<Storage, "getItem" | "setItem">;

const PREFIX = "graph-studio.settings.";
const LAST = "graph-studio.last-source";

function hashOf(text: string): string {
  let hash = 0x811c9dc5;
  for (let at = 0; at < text.length; at += 1) hash = Math.imul(hash ^ text.charCodeAt(at), 0x01000193) >>> 0;
  return hash.toString(16);
}

/** The storage key of one source: its identity, not its text, when the text is long. */
export function sourceKey(source: Source): string {
  if (source.kind === "document") return `${PREFIX}document:${source.name}:${source.text.length}:${hashOf(source.text)}`;
  return `${PREFIX}${JSON.stringify(source)}`;
}

function read(storage: SettingsStorage, key: string): Settings | null {
  try {
    const text = storage.getItem(key);
    if (text === null) return null;
    const value: unknown = JSON.parse(text);
    return readSettings(value);
  } catch {
    return null;
  }
}

export function recall(storage: SettingsStorage, source: Source): Settings | null {
  return read(storage, sourceKey(source));
}

export function remember(storage: SettingsStorage, settings: Settings): void {
  try {
    const key = sourceKey(settings.source);
    storage.setItem(key, JSON.stringify(settings));
    storage.setItem(LAST, key);
  } catch {
    // Nothing to do: the drawing is right, only its memory is gone.
  }
}

/** The settings the studio opens with: the last source's, or the defaults. */
export function openingSettings(storage: SettingsStorage | null): Settings {
  if (storage === null) return DEFAULT_SETTINGS;
  try {
    const key = storage.getItem(LAST);
    return (key === null ? null : read(storage, key)) ?? DEFAULT_SETTINGS;
  } catch {
    return DEFAULT_SETTINGS;
  }
}

/** Writes the settings each time the store holds a different document; returns what stops it. */
export function keepSettings(store: Store<StudioState>, storage: SettingsStorage): () => void {
  let last = store.get().settings;
  return store.subscribe(() => {
    const { settings } = store.get();
    if (settings === last) return;
    last = settings;
    remember(storage, settings);
  });
}

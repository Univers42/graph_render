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
export type PageExits = Pick<EventTarget, "addEventListener" | "removeEventListener">;

const PREFIX = "graph-studio.settings.";
const LAST = "graph-studio.last-source";
/** The key of the source the page holds, cleared when the page ends cleanly. */
const PENDING = "graph-studio.pending-source";

/**
 * The longest document whose settings are stored. A page's storage holds about 5 MB, and the
 * settings carry the text, so a longer one would be hashed and copied on every settings change
 * only for the write to fail.
 */
export const STORED_DOCUMENT_CHARS = 2 ** 20;

function hashOf(text: string): string {
  let hash = 0x811c9dc5;
  for (let at = 0; at < text.length; at += 1) hash = Math.imul(hash ^ text.charCodeAt(at), 0x01000193) >>> 0;
  return hash.toString(16);
}

/** The storage key of one source, or null for a document too long to store. */
export function sourceKey(source: Source): string | null {
  if (source.kind !== "document") return `${PREFIX}${JSON.stringify(source)}`;
  if (source.text.length > STORED_DOCUMENT_CHARS) return null;
  return `${PREFIX}document:${source.name}:${source.text.length}:${hashOf(source.text)}`;
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

/** Sets the storage's `key` and keeps quiet when it cannot: only the memory of the drawing is lost. */
function put(storage: SettingsStorage, key: string, value: string): void {
  try {
    storage.setItem(key, value);
  } catch {
    // Nothing to do: the drawing is right, only its memory is gone.
  }
}

export function recall(storage: SettingsStorage, source: Source): Settings | null {
  const key = sourceKey(source);
  return key === null ? null : read(storage, key);
}

export function remember(storage: SettingsStorage, settings: Settings): void {
  const key = sourceKey(settings.source);
  if (key === null) return;
  put(storage, key, JSON.stringify(settings));
  put(storage, LAST, key);
}

/**
 * The settings the studio opens with: the last source's, or the defaults when there is none or
 * the page that held it never ended cleanly. A graph that took the page down with it is
 * therefore not opened again by itself; picked again, it is recalled as usual.
 *
 * Caveat: "did not end cleanly" also covers a browser killed from outside or a host reset, which
 * skip an innocent last source once; and two studio pages share one marker, so the first to close
 * clears the other's.
 */
export function openingSettings(storage: SettingsStorage | null): Settings {
  if (storage === null) return DEFAULT_SETTINGS;
  try {
    const key = storage.getItem(LAST);
    if (key === null || storage.getItem(PENDING) === key) return DEFAULT_SETTINGS;
    storage.setItem(PENDING, key);
    return read(storage, key) ?? DEFAULT_SETTINGS;
  } catch {
    return DEFAULT_SETTINGS;
  }
}

function isPage(page: Partial<PageExits>): page is PageExits {
  return typeof page.addEventListener === "function" && typeof page.removeEventListener === "function";
}

/** The page's own events, or null where there are none (a worker, or node under test). */
function hostPage(): PageExits | null {
  const page: Partial<PageExits> = globalThis;
  return isPage(page) ? page : null;
}

/**
 * Writes the settings each time the store holds a different document, and marks the source as
 * held until the page ends (`pagehide`) or this stops; returns what stops it.
 */
export function keepSettings(store: Store<StudioState>, storage: SettingsStorage, page = hostPage()): () => void {
  let last = store.get().settings;
  const ended = (): void => put(storage, PENDING, "");
  page?.addEventListener("pagehide", ended);
  const unsubscribe = store.subscribe(() => {
    const { settings } = store.get();
    if (settings === last) return;
    const held = settings.source === last.source ? null : sourceKey(settings.source);
    last = settings;
    if (held !== null) put(storage, PENDING, held);
    remember(storage, settings);
  });
  return () => {
    unsubscribe();
    page?.removeEventListener("pagehide", ended);
    ended();
  };
}

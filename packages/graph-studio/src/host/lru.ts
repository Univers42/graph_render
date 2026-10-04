/**
 * A least-recently-used cache over a `Map`, whose iteration order is insertion order: a read
 * moves its key to the end, and a write past `capacity` drops the first key.
 *
 * Caveat: it holds what it was given and never re-validates it. A value that changed at its
 * source reads stale until it is deleted, cleared or evicted.
 */
export interface Lru<Value> {
  get(key: string): Value | undefined;
  set(key: string, value: Value): void;
  delete(key: string): void;
  clear(): void;
  readonly size: () => number;
}

export function createLru<Value>(capacity: number): Lru<Value> {
  const entries = new Map<string, Value>();
  return {
    get: (key) => {
      const value = entries.get(key);
      if (value === undefined) return undefined;
      entries.delete(key);
      entries.set(key, value);
      return value;
    },
    set: (key, value) => {
      entries.delete(key);
      entries.set(key, value);
      if (entries.size <= capacity) return;
      const oldest = entries.keys().next();
      if (oldest.done !== true) entries.delete(oldest.value);
    },
    delete: (key) => void entries.delete(key),
    clear: () => entries.clear(),
    size: () => entries.size,
  };
}

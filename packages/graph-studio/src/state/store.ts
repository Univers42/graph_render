/** One value, read synchronously and announced on change: what `useSyncExternalStore` wants. */

export interface Store<Value> {
  readonly get: () => Value;
  readonly set: (next: Value) => void;
  readonly update: (change: (current: Value) => Value) => void;
  /** Returns what stops the listening. */
  readonly subscribe: (listener: () => void) => () => void;
}

export function createStore<Value>(initial: Value): Store<Value> {
  let current = initial;
  const listeners = new Set<() => void>();
  const set = (next: Value): void => {
    if (Object.is(next, current)) return;
    current = next;
    for (const listener of [...listeners]) listener();
  };
  return {
    get: () => current,
    set,
    update: (change) => set(change(current)),
    subscribe: (listener) => {
      listeners.add(listener);
      return () => void listeners.delete(listener);
    },
  };
}

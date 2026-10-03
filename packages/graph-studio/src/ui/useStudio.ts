import { useRef, useSyncExternalStore } from "react";

import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";

/** The whole state, or the whole state again: one value, replaced, never edited. */
export function useStudioState(studio: Studio): StudioState {
  return useSyncExternalStore(studio.store.subscribe, studio.store.get, studio.store.get);
}

/**
 * The value already held when the new one is equal to it, so an equal selection keeps its
 * reference. `undefined` is never held: a selection of `undefined` is simply selected again,
 * and every field of the state is an object or `null` in any case.
 */
export function sameWhenEqual<T>(held: T | undefined, next: T, isEqual: (a: T, b: T) => boolean): T {
  return held !== undefined && isEqual(held, next) ? held : next;
}

/**
 * One slice of the state, and the same reference for as long as that slice is equal.
 *
 * WHY the held reference: a selector that built its own object would hand React a new
 * snapshot on every read and loop for ever. `isEqual` is for the slices that are rebuilt
 * rather than replaced.
 */
export function useStudioSelector<T>(
  studio: Studio,
  select: (state: StudioState) => T,
  isEqual: (a: T, b: T) => boolean = Object.is,
): T {
  const held = useRef<T | undefined>(undefined);
  // WHY the selection is the snapshot: React compares snapshots with Object.is and skips the
  // render when they match, so a store change to a slice this component does not read
  // costs one select and one compare, not a render.
  const snapshot = (): T => {
    const value = sameWhenEqual(held.current, select(studio.store.get()), isEqual);
    held.current = value;
    return value;
  };
  return useSyncExternalStore(studio.store.subscribe, snapshot, snapshot);
}

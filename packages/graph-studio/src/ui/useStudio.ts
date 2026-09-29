import { useSyncExternalStore } from "react";

import type { StudioState } from "../state/model.ts";
import type { Studio } from "../studio/studio.ts";

/** The whole state, or the whole state again: one value, replaced, never edited. */
export function useStudioState(studio: Studio): StudioState {
  return useSyncExternalStore(studio.store.subscribe, studio.store.get, studio.store.get);
}

/**
 * Starts the live loop after every force layout run, and feeds the bar the calls in flight.
 * Split from `bridge.ts`, which owns what the loop says back.
 */
import type { LiveBridge } from "./bridge.ts";
import { settlesLive } from "./live.ts";
import type { Store } from "../state/store.ts";

/** What `watchRuns` reads of the studio's state: the calls in flight and the last run drawn. */
export interface RunState {
  readonly busy: readonly unknown[];
  readonly run: { readonly layoutId: string } | null;
}

/**
 * A force layout settles live: the loop steps the session the run left, which keeps the run's
 * picture, or settles a large graph's scatter on screen (`settle.ts`). Every other layout is
 * finished, so nothing starts. A batch layout
 * run shows the same strip with no fraction of its own — one call, no progress inside it.
 *
 * Keyed on the run, not on its layout id: a large graph reports `particle_mesh` whichever force
 * layout was asked for, so two runs in a row can share an id. Keyed on the id, picking
 * `particle_mesh` and then opening 400k nodes left the scatter unsettled (0 frames, 2026-10-03).
 */
export function watchRuns(store: Pick<Store<RunState>, "get" | "subscribe">, bridge: LiveBridge): () => void {
  let seen: RunState["run"] = null;
  let wasBusy = 0;
  return store.subscribe(() => {
    const at = store.get();
    if (at.busy.length !== wasBusy) {
      wasBusy = at.busy.length;
      bridge.batch(wasBusy);
    }
    if (at.run === seen) return;
    seen = at.run;
    if (at.run !== null && settlesLive(at.run.layoutId)) bridge.start();
  });
}

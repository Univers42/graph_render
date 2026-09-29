/**
 * The animate action's clock: nodes appear in ingest order over a duration. It owns the one
 * timer, so a second start replaces the first and a cancel leaves what showed showing.
 *
 * Ponytail: steps are timers, not frames, so a slow page reveals in fewer, larger jumps; the
 * count at each step is right for the time that has passed, never for the frames drawn.
 */
import type { Outcome } from "../actions/registry.ts";

export interface RevealDeps {
  readonly total: () => number;
  /** How many nodes show, or null for all of them. */
  readonly show: (count: number | null) => void;
  readonly now: () => number;
  /** Runs `step` after a pause; the returned function cancels it. */
  readonly schedule: (step: () => void, ms: number) => () => void;
}

export interface Reveal {
  start(durationMs: number): Outcome;
  cancel(): Outcome;
}

const STEP_MS = 40;

export function createReveal(deps: RevealDeps): Reveal {
  let stop: (() => void) | null = null;
  let shown = 0;
  const halt = (): void => {
    stop?.();
    stop = null;
  };
  const step = (from: number, duration: number): void => {
    const total = deps.total();
    shown = Math.min(total, Math.floor((total * (deps.now() - from)) / duration));
    if (shown >= total) {
      stop = null;
      deps.show(null);
      return;
    }
    deps.show(shown);
    stop = deps.schedule(() => step(from, duration), STEP_MS);
  };
  return {
    start: (durationMs) => {
      halt();
      const total = deps.total();
      if (durationMs <= 0 || total === 0) {
        deps.show(null);
        return { message: `${total} nodes shown` };
      }
      shown = 0;
      deps.show(0);
      const from = deps.now();
      stop = deps.schedule(() => step(from, durationMs), STEP_MS);
      return { message: `revealing ${total} nodes over ${durationMs} ms` };
    },
    cancel: () => {
      if (stop === null) return { message: "nothing is being revealed" };
      halt();
      return { message: `stopped at ${shown} of ${deps.total()} nodes` };
    },
  };
}

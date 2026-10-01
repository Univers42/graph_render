/**
 * The watchdog over a settle the worker walked away from: the strip is up, the panel says a
 * session is live, and nothing will ever say otherwise.
 *
 * Ponytail: one named bound and a timer the messages re-arm, rather than a poll over a clock —
 * the worker already says it is alive, so there is nothing to sample.
 */

/**
 * How long the strip may stay up with no word from the worker before the session is called
 * dead. Long enough that a late frame is not a death, short enough that a person watching a
 * dead settle is not still watching it a minute later.
 *
 * Caveat: this is a bound, not a measurement — a live motor on a very large graph that goes
 * more than SILENCE_MS between two messages is declared dead while it is still working. The
 * verdict is not sticky: the next message re-arms the watchdog and the session is live again.
 */
export const SILENCE_MS = 4_000;

/** Why a session ended with no message at all. */
export const SILENCE_REASON = "the motor worker stopped answering";

/** Why a session ended with a worker `error` event, in the event's own words. */
export function workerFailed(detail: string): string {
  return detail === "" ? "the motor worker failed" : `the motor worker failed: ${detail}`;
}

export interface WatchdogDeps {
  /** Runs `run` after `ms`, and returns what cancels it. */
  readonly schedule: (run: () => void, ms: number) => () => void;
}

export interface Watchdog {
  /** The strip is up and a message just arrived: the bound starts again from here. */
  readonly touch: () => void;
  /** Nothing is being shown: the watchdog sleeps. */
  readonly rest: () => void;
  /** The worker failed in the page's hearing: the same path, at once. */
  readonly lost: (detail: string) => void;
  /** Stops the watchdog for good. */
  readonly stop: () => void;
}

/** `setTimeout`, in the shape the watchdog wants, so a test can hand it its own. */
export const later = (run: () => void, ms: number): (() => void) => {
  const timer = setTimeout(run, ms);
  return () => clearTimeout(timer);
};

/**
 * Failing input: a schedule that fires early, or a message that lands between the cancel and
 * the re-arm, would count silence that never happened — so the timer is always re-armed from
 * the last message and never left to run down. Direction: the bound is measured from the last
 * message, not from when the settle started, so a long but talking settle is never a dead one.
 */
export function createWatchdog(lost: (reason: string) => void, deps: WatchdogDeps): Watchdog {
  let cancel: (() => void) | null = null;
  const stop = (): void => {
    cancel?.();
    cancel = null;
  };
  const fire = (reason: string): void => {
    stop();
    lost(reason);
  };
  return {
    touch: () => {
      stop();
      cancel = deps.schedule(() => fire(SILENCE_REASON), SILENCE_MS);
    },
    rest: stop,
    lost: (detail) => fire(workerFailed(detail)),
    stop,
  };
}

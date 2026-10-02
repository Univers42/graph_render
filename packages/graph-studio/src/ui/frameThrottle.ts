/**
 * One redraw per frame, for a store that publishes once per frame of its own.
 *
 * A live settle pushes the bar about sixty times a second. Subscribing to that with
 * `useSyncExternalStore` redraws whatever the subscriber returns, and at panel level that is
 * four sliders, four buttons and the reason line, sixty times a second, for one number. So
 * the answer is not read faster than a frame can show: publishes collapse into one redraw.
 *
 * The scheduler is injected rather than reached for, so the collapse is measured against a
 * scheduler the test drives itself — node has no `requestAnimationFrame`, and a worker has
 * none either, which is why the fallback is a real part of this module and not a stub.
 */
import type { Bar } from "./progress.ts";

/** What a host needs to be asked for a frame: both halves, or neither. */
export interface FrameHost {
  /** Absent in a rig with no frames: the node test rig, and the motor's own worker. */
  readonly requestAnimationFrame?: (run: () => void) => number;
  /** The token `requestAnimationFrame` handed back, given up again. */
  readonly cancelAnimationFrame?: (token: number) => void;
}

/** Asks for the next frame and returns the call that takes it back. */
export interface FrameScheduler {
  readonly next: (run: () => void) => () => void;
}

/** The bar's store: a value, and the change to it. */
export interface BarStore {
  readonly bar: () => Bar;
  readonly onBar: (handler: (bar: Bar) => void) => () => void;
}

/** Sixty hertz: what a frame is, for a host whose only clock is a timer. */
export const FRAME_MS = 16;

const NOTHING = (): void => undefined;

/**
 * WHY the timer is 16 ms and not 0: a 0 ms timer runs on the next turn of the loop, so every
 * publish would get a redraw of its own and the bound this module exists to keep would be
 * gone. One display period is what a frame is, so a host with no frames still redraws as
 * fast as a frame can be shown.
 */
function timerFrames(): FrameScheduler {
  return {
    next: (run) => {
      const token = setTimeout(run, FRAME_MS);
      return () => clearTimeout(token);
    },
  };
}

/**
 * WHY the host is a parameter with `globalThis` behind it: a test rig hands in frames it
 * drives itself, and a host is asked rather than read so that one which has neither half of
 * a frame request falls back instead of calling something that is not there.
 */
export function frameScheduler(host: FrameHost = globalThis): FrameScheduler {
  const { requestAnimationFrame: request, cancelAnimationFrame: drop } = host;
  if (request === undefined || drop === undefined) return timerFrames();
  return {
    next: (run) => {
      const token = request(run);
      return () => drop(token);
    },
  };
}

/**
 * Subscribes to the bar and redraws at most once a frame; the returned call ends it.
 *
 * WHY the redraw reads the store itself and is handed nothing: the store publishes its own
 * value to whoever wants it, and a redraw that arrives after the last publish must show the
 * last publish rather than the one that happened to be in flight.
 *
 * Caveat: `drop` is the frame waiting, and it can only be given up once the scheduler has
 * said what the frame is called. A scheduler that runs its callback before `next` returns
 * therefore leaves the throttle latched and every later publish is dropped; the two
 * schedulers here (a frame request, a timer) are both asynchronous, and the test's is too.
 */
export function throttledBar(store: BarStore, scheduler: FrameScheduler, publish: () => void): () => void {
  let drop: (() => void) | null = null;
  const off = store.onBar(() => {
    if (drop !== null) return;
    drop = scheduler.next(() => {
      drop = null;
      publish();
    });
  });
  return () => {
    off();
    const waiting = drop ?? NOTHING;
    drop = null;
    waiting();
  };
}
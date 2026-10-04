/**
 * One run per frame, for a control that can be moved faster than a frame can be shown.
 *
 * A slider that commits on every pointer move would ask the motor for the same layout a hundred
 * times a second; one that waited for a pause would feel dead. So the changes pile up and the
 * run happens once, on the next frame, with all of them in it.
 *
 * The scheduler is injected rather than reached for, so the collapse is measured against a
 * scheduler the test drives itself — node has no `requestAnimationFrame` (`frameThrottle.ts`).
 */
import type { FrameScheduler } from "./frameThrottle.ts";

export interface OneRun {
  /** Ask for a run; several asks before the frame comes cost one run. */
  readonly ask: () => void;
  /** Give up the frame waiting, and ask nothing more. */
  readonly drop: () => void;
}

const NOTHING = (): void => undefined;

export function oneRunPerFrame(scheduler: FrameScheduler, run: () => void): OneRun {
  let waiting: (() => void) | null = null;
  return {
    ask: () => {
      if (waiting !== null) return;
      waiting = scheduler.next(() => {
        waiting = null;
        run();
      });
    },
    drop: () => {
      const frame = waiting ?? NOTHING;
      waiting = null;
      frame();
    },
  };
}

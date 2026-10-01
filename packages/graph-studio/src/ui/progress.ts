/**
 * The progress bar at the top of the canvas: one number, one state machine, no DOM.
 *
 * The bar is fed two things and shows one. A live settle reports alpha, which cools
 * geometrically towards `ALPHA_MIN`, so the fraction is alpha read as a remainder. A batch
 * layout run in the worker reports only that it is busy — one call, no progress inside it —
 * so its bar is shown with no fraction and the CSS sweeps it.
 *
 * Ponytail: the fraction is `alpha` read as a remainder over `[ALPHA_MIN, 1]`, not the tick
 * that will be reached next, so the bar never quite reaches zero before the loop stops.
 * Failing input: an alpha that is not a finite number hides the bar rather than painting a
 * NaN width. Direction: the bar drains, so it reads as "how much is left". Escape hatch:
 * `ALPHA_MIN` is the loop's own stop threshold, imported from liveLoop.ts rather than copied,
 * so the bar empties exactly when the loop stops.
 */
import { ALPHA_MIN } from "../motor/liveLoop.ts";
import type { ForceFrame } from "../motor/protocol.ts";

export { ALPHA_MIN };

export interface Bar {
  readonly visible: boolean;
  /** What is left, 0..1; `null` when the work reports no progress of its own. */
  readonly fraction: number | null;
  readonly label: string;
}

export const HIDDEN: Bar = { visible: false, fraction: null, label: "" };

function clamp(value: number): number {
  return value < 0 ? 0 : value > 1 ? 1 : value;
}

/** The settle of a live simulation: alpha down towards `ALPHA_MIN`. */
export function settleBar(alpha: number, running: boolean): Bar {
  if (!running || !Number.isFinite(alpha)) return HIDDEN;
  const span = 1 - ALPHA_MIN;
  return {
    visible: true,
    fraction: span <= 0 ? 0 : clamp((alpha - ALPHA_MIN) / span),
    label: "settling",
  };
}

/** The work the worker is doing: how many calls are running, and no fraction. */
export function batchBar(busy: number): Bar {
  if (!Number.isFinite(busy) || busy < 1) return HIDDEN;
  return { visible: true, fraction: null, label: `${busy} running` };
}

/** What a frame from the live loop shows. */
export function frameBar(frame: ForceFrame): Bar {
  return settleBar(frame.alpha, frame.running);
}
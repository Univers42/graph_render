/**
 * How many edges a moving 2D frame draws, paced by what the last moving frame cost. A fixed
 * MOVING_BUDGET of 16 000 left a 20 000-node random graph at 19.8 fps under software raster
 * with 2.27 ms of script per frame: the cost is the raster the browser runs after the frame
 * callback returns, which no timer inside the callback sees. At 2048 the same pan ran at
 * 55.8 fps (target/studio-perf/p5c2d and p5c2d-2048). So a moving frame is timed from its
 * own animation-frame stamp to the next one, which includes that raster.
 *
 * Caveat: the gap also holds whatever else the page did in between (a GC, a layout, another
 * tab's work), so one hiccup halves the budget for a frame that was not at fault; the budget
 * climbs back only after RECOVER_FRAMES on-time frames, so the view draws fewer edges than it
 * could for about half a second. And on a budget that sits one doubling below its ceiling,
 * every recovery attempt costs one late frame (about one in 31).
 */
import { MOVING_FLOOR, SLOW_MS, nextBudget } from "../webgl2/plan.ts";
import { MOVING_BUDGET } from "./edges.ts";

/** A gap below this is a frame that met a 60 Hz deadline with room to spare. */
const ON_TIME_MS = 20;
const RECOVER_FRAMES = 30;

export interface Pace {
  budget: number;
  /** Consecutive on-time moving frames since the budget last changed. */
  onTime: number;
  /** True while a probe for the next animation frame is pending. */
  probing: boolean;
}

export function newPace(): Pace {
  return { budget: MOVING_BUDGET, onTime: 0, probing: false };
}

/** Folds the gap after one moving frame into the budget, between MOVING_FLOOR and MOVING_BUDGET. */
export function paced(pace: Pace, gapMs: number): void {
  if (gapMs > SLOW_MS) {
    pace.budget = nextBudget(pace.budget, gapMs, MOVING_BUDGET);
    pace.onTime = 0;
    return;
  }
  pace.onTime = gapMs < ON_TIME_MS ? pace.onTime + 1 : 0;
  if (pace.onTime < RECOVER_FRAMES || pace.budget >= MOVING_BUDGET) return;
  pace.onTime = 0;
  pace.budget = Math.min(MOVING_BUDGET, pace.budget * 2);
}

/** True when a moving frame's cost should feed the budget: a 2D frame the canvas drew with enough edges to thin. */
export function worthPacing(edgeCount: number, drewOnGpu: boolean, flat: boolean): boolean {
  return flat && !drewOnGpu && edgeCount > MOVING_FLOOR;
}

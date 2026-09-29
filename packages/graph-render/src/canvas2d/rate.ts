/**
 * The rate the view reports. Only frames painted while the view MOVES are counted (a pan, a
 * zoom, a layout transition): the view paints on demand, so the time between two frames
 * painted for two hovers is the time between the hovers. Counted over every frame, a
 * parked graph read "4.5 fps".
 *
 * Ponytail: this is the rate of frames painted, so it is capped by how often the browser
 * calls back and by how often the pointer reports a move; a view that could paint 200
 * frames a second reads 60. It never reads higher than the truth. The script time of the
 * last frame is reported next to it and has no such cap.
 */

const STAMPS = 32;
const WINDOW_MS = 1000;
const PARKED_MS = 400;

export interface Rate {
  /** `requestAnimationFrame` times of the last moving frames; 0 is an empty slot. */
  readonly stamps: Float64Array;
  count: number;
}

export function newRate(): Rate {
  return { stamps: new Float64Array(STAMPS), count: 0 };
}

export function stamp(rate: Rate, now: number, moving: boolean): void {
  if (!moving) return;
  rate.stamps[rate.count % STAMPS] = now;
  rate.count += 1;
}

/** Frames per second over the last second; 0 when nothing moved in the last 400 ms. */
export function fpsOf(rate: Rate, now: number): number {
  let count = 0;
  let oldest = now;
  let newest = 0;
  for (const at of rate.stamps) {
    if (at === 0 || now - at > WINDOW_MS) continue;
    count += 1;
    oldest = Math.min(oldest, at);
    newest = Math.max(newest, at);
  }
  if (count < 2 || now - newest > PARKED_MS) return 0;
  return ((count - 1) * 1000) / (newest - oldest);
}

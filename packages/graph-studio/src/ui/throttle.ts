/** Whether a frame may write the HUD: how long since the last write that was. */

/** `now - last >= every`; `-Infinity` as `last` makes the first frame due at once. */
export function due(last: number, now: number, every: number): boolean {
  return now - last >= every;
}

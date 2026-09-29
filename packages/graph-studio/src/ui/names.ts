/** The short strings the chrome prints, so no panel grows its own idea of a number. */

const DASH = "—";
const NO_DIGIT = 8;
const EVERY_MS = 1000;
const SIGFIGS = 3;

/**
 * Ponytail: the family is dropped, so `a.b` and `c.b` are both shown as `b` and a list of
 * choices holds the same word twice. Escape hatch: the full id is in the console line and in
 * the log, and a list that shows one family at a time has no repeats in it.
 */
export function shortName(id: string): string {
  const dot = id.indexOf(".");
  return dot < 0 ? id : id.slice(dot + 1);
}

/** Enough of a digest to recognise a drawing and no more; nothing drawn reads as a dash. */
export function digest8(digest: string | null): string {
  return digest === null ? DASH : digest.slice(0, NO_DIGIT);
}

/**
 * Ponytail: a duration from 999.5 ms rounds to `1000 ms` and not to `1.0 s`, so a run that
 * takes a second looks like a second before it looks like a thousand. Escape hatch: the
 * seconds column of the HUD, which always shows seconds past a second.
 */
export function ms(value: number): string {
  if (value < 1) return "<1 ms";
  if (value < EVERY_MS) return `${Math.round(value)} ms`;
  return `${(value / EVERY_MS).toFixed(1)} s`;
}

/**
 * Ponytail: three significant digits is not the value, and two neighbours whose weights
 * differ in the fourth digit read the same here. Escape hatch: the console prints the value
 * an analysis reported, where nothing is rounded.
 */
export function sig3(value: number): string {
  return String(Number(value.toPrecision(SIGFIGS)));
}

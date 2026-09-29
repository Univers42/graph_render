/**
 * Seeded input for the property tests: one number in, the same draws out, so a red run
 * replays from the number the test printed. The PRNG is the render package's own
 * (support.ts:5), not a second one, so a seed means the same thing on both sides.
 *
 * Ponytail: every generator here draws from a fixed table of fragments, so it can only
 * produce what is in the table and it cannot shrink a failure — a red run hands you the
 * whole line, not the three characters inside it. It also cannot produce an input shaped
 * by anything the table lacks: the 4 KB line, the lone surrogates and the NUL bytes are
 * reachable only because the cases below ask for them, so a bug that needs "a 4 KB line
 * whose 2000th character is an unclosed quote" is out of reach. It under-covers, so a
 * green run means "nothing in the table broke", not "the parser is safe". Escape hatch:
 * add the fragment to CONSOLE_ALPHABET and re-run — same seed, new corpus, so a new
 * fragment is a new experiment rather than a new seed to remember.
 */
import { mulberry32 } from "../../graph-render/tests/support.ts";

export { mulberry32 };

export interface Stream {
  /** The raw draw: `[0, 1)`. Use it to ask a question the helpers do not. */
  next(): number;
  /** Inclusive on both ends, and never below `min`. */
  int(min: number, max: number): number;
  /** One of `values`, or a thrown error when the table is empty. Never `undefined`. */
  pick(values: readonly string[]): string;
}

export function stream(seed: number): Stream {
  const next = mulberry32(seed);
  const int = (min: number, max: number): number => min + Math.floor(next() * (max - min + 1));
  const pick = (values: readonly string[]): string => {
    const drawn = values[Math.floor(next() * values.length)];
    if (drawn === undefined) throw new Error("fuzz: drew from an empty table");
    return drawn;
  };
  return { next, int, pick };
}

/**
 * The console's own alphabet. Every class a console line can be in is a row here: a
 * delimiter, a quote, a backslash, whitespace, control characters, NUL, non-ASCII letters,
 * astral-plane emoji, lone surrogates, and the studio's real command words and parameter
 * names. parse-fuzz.test.ts fails if a class goes missing, so the table cannot quietly
 * stop covering something the property claims to cover.
 */
export const CONSOLE_ALPHABET: readonly string[] = [
  " ", "\t", "\n", "\r", "\v", "\f", "\u0000", "\u001b", "\u007f", "\u00a0",
  "\"", "'", "\\", "=", ":", "-", "_", ".", "/", "#",
  "*", "?", ";", "|", "&", "$", "`", "~", "!", "%", "^",
  "(", ")", "[", "]", "{", "}", "<", ">", "@", "+",
  "a", "b", "z", "A", "M", "0", "1", "7", "9", ".",
  "-1", "0.5", "500", "1e999", "NaN", "Infinity",
  "é", "ü", "ß", "ñ", "Ж", "א", "م", "ह", "こ", "한",
  "\u2192", "\u00a7", "\u00b6", "\u00ab", "\u00bb", "\ufeff", "\u00bf", "\u2028", "\u2029",
  "😀", "👍", "🧑🏽‍💻", "🇫🇷", "👨‍👩‍👧‍👦", "🏳️‍🌈",
  "\ud800", "\udfff", "\ud83d", "\udc00", "a\ud800b", "\ud83d\ude00",
  "\"a b\"", "'x y'", "\"", "'", "\"\"", "''", "\"\\\"", "'\\'",
  "layout", "synthetic", "filter", "fit", "png", "zoom", "pan",
  "labels", "edges", "theme", "focus", "help", "recipe", "snapshot",
  "fixture", "analysis", "colour", "scale", "size", "group", "mindegree",
  "document", "replay", "reset", "deselect", "cancel", "clear", "unfilter",
  "id=", "nodes=", "degree=", "seed=", "text=", "name=", "by=",
  "mode=", "shape=", "hide=", "path=", "factor=", "min=", "node=", "command=",
  "layout.grid", "layout.force.barnes_hut", "layout.forceatlas2", "layout.none",
];

/** A line of `0..maxParts` fragments joined together: mostly short, sometimes not. */
export function line(draws: Stream, maxParts = 12): string {
  const parts: string[] = [];
  const count = draws.int(0, maxParts);
  for (let at = 0; at < count; at += 1) parts.push(draws.pick(CONSOLE_ALPHABET));
  return parts.join("");
}

/**
 * A line of at least `chars` characters, drawn the same way `line` draws but without
 * stopping. A 4 KB line is only reachable through here: `line` caps its parts, so the
 * long-input path in the parser is never entered by the ordinary draws.
 */
export function longLine(draws: Stream, chars: number): string {
  let text = "";
  while (text.length < chars) text += draws.pick(CONSOLE_ALPHABET);
  return text;
}

/**
 * The query scanner: words with the column each one starts at, the three keywords, and the two
 * parens. A quote always begins a word of its own, which is what keeps `"two:words"` a word
 * instead of a `field:value`, and the printer leans on that to quote anything ambiguous.
 *
 * It never throws. An unclosed quote is the one thing the scan cannot recover from, so it comes
 * back as a `failure` and the parser is the one that refuses, in its own voice.
 */

/** A bare word, or a quoted one. A quoted word is never read as `field:value`. */
export interface QueryWord {
  readonly type: "word";
  readonly text: string;
  readonly quoted: boolean;
  readonly column: number;
}

export interface QueryKey {
  readonly type: "and" | "or" | "not";
  readonly column: number;
}

export interface QueryParen {
  readonly type: "open" | "close";
  readonly column: number;
}

export type QueryToken = QueryWord | QueryKey | QueryParen;

/** Where an unclosed quote began, so the refusal can point at the quote and not at the end. */
export interface QueryFailure {
  readonly column: number;
  readonly quote: string;
}

export interface QueryScan {
  readonly tokens: readonly QueryToken[];
  readonly failure: QueryFailure | null;
}

const KEYWORDS: ReadonlyMap<string, "and" | "or" | "not"> = new Map([
  ["and", "and"],
  ["or", "or"],
  ["not", "not"],
]);

interface Scan {
  readonly tokens: QueryToken[];
  failure: QueryFailure | null;
}

interface Bare {
  readonly value: string;
  readonly end: number;
}

/**
 * A `\` only escapes inside a double-quoted run, as in the command scanner, so a bare word
 * holding one reads as itself and the printer may leave it bare.
 *
 * Ponytail: `tag:a\b` is read as the three characters it is, not as `ab`. It errs towards
 * reading more as literal, so a path pasted unquoted survives. The escape hatch is the quote:
 * `"a\\b"` is how a backslash is written when a word means something other than itself.
 */
function readQuoted(scan: Scan, text: string, opened: number): number {
  const quote = text[opened] ?? '"';
  let at = opened + 1;
  let value = "";
  while (at < text.length) {
    const char = text[at] ?? "";
    if (char === quote) {
      scan.tokens.push({ type: "word", text: value, quoted: true, column: opened + 1 });
      return at + 1;
    }
    const escaped = char === "\\" && quote === '"' && at + 1 < text.length;
    value += escaped ? (text[at + 1] ?? "") : char;
    at += escaped ? 2 : 1;
  }
  scan.failure = { column: opened + 1, quote };
  return at;
}

function readBare(text: string, from: number): Bare {
  let at = from;
  let value = "";
  while (at < text.length) {
    const char = text[at] ?? "";
    if (/\s/.test(char) || char === "(" || char === ")" || char === '"' || char === "'") break;
    value += char;
    at += 1;
  }
  return { value, end: at };
}

/**
 * A bare word equal to a keyword, in any case, is that keyword; a quoted one is a word.
 *
 * Ponytail: `a AND` is a keyword whatever the user meant by the letters, so a label search for
 * the word `and` needs the quotes. It errs towards reading more as grammar, and the escape
 * hatch is the quote, which is also what the printer writes for such a word.
 */
function pushWord(scan: Scan, value: string, column: number, quoted: boolean): void {
  if (!quoted && KEYWORDS.has(value.toLowerCase())) {
    scan.tokens.push({ type: KEYWORDS.get(value.toLowerCase()) ?? "and", column });
    return;
  }
  scan.tokens.push({ type: "word", text: value, quoted, column });
}

function take(scan: Scan, text: string, at: number): number {
  const char = text[at] ?? "";
  if (/\s/.test(char)) return at + 1;
  const column = at + 1;
  if (char === "(" || char === ")") {
    scan.tokens.push({ type: char === "(" ? "open" : "close", column });
    return at + 1;
  }
  if (char === '"' || char === "'") return readQuoted(scan, text, at);
  const bare = readBare(text, at);
  pushWord(scan, bare.value, column, false);
  return bare.end;
}

export function scanQuery(text: string): QueryScan {
  const scan: Scan = { tokens: [], failure: null };
  let at = 0;
  while (at < text.length && scan.failure === null) at = take(scan, text, at);
  return { tokens: scan.tokens, failure: scan.failure };
}

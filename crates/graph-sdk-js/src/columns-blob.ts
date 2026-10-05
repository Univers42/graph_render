// The string table's **blob**: where it starts, how wide each entry is, and the one call that
// puts the bytes there. Split out of `columns-assemble.ts`, which was over the house's line limit
// once this file's subject — and its two measuring paths — grew a module of its own.
//
// Two **measuring** paths, and which one runs is decided by **one pass that measures nothing**:
//
//   * every code unit ASCII — each entry's width is its code-unit count;
//   * anything wider — every entry is walked and measured with {@link utf8Length}.
//
// **Both are placed the same way**, by one `encodeInto` over the joined text: the measuring
// pass is where the width classes and the refusals live, and the writing pass is one call.
// The studio's own data — icons `🌿` and `📈` — is on the second measuring path, and
// `docs/measurements/perf-p4g-wasm.md` is where both the per-entry placement this replaced
// and the join P4f removed are measured.
//
// **Caveat:** the ASCII test is a code-unit scan, so a string that is *well-formed but wide*
// (an emoji, an `é`) takes the exact measuring path, and a table of only-ASCII takes the
// cheap one. Both produce the same document; only the measuring cost differs.

import { GraphMotorError } from "./errors.ts";

/** Why a string cannot be encoded as UTF-8: the reason both encoders give for it. */
export const NOT_WELL_FORMED =
  "is not well-formed: a lone surrogate cannot be encoded as UTF-8 and would read back as a " +
  "different id (U+FFFD), so the document is refused instead";

/** A value the encoder refuses, with the field that holds it and why. It lives here, beside the
 *  blob placement that raises most of them, and `columns-assemble.ts` re-exports it: the package's
 *  public name for it is unchanged, only the module it is written in. */
export class ColumnsEncoderError extends GraphMotorError {
  /** The dotted field path, e.g. `nodes[7].icon` — a caller can branch on this. */
  readonly field: string;

  /** `reason` finishes the sentence that starts with the field, e.g. {@link NOT_WELL_FORMED}. */
  constructor(field: string, reason: string) {
    super(`\`${field}\` ${reason}`);
    this.field = field;
  }
}

/** One `TextEncoder` for the whole module: `encode` would allocate an array per entry, and
 *  `encodeInto` writes where it is told to. */
const ENCODER = new TextEncoder();

/** Where the blob starts: the header, then the offsets table. */
export function blobAt(stringCount: number): number {
  return 32 + 4 * (stringCount + 1);
}

/** Where the columns start: the blob, padded with zeros to a multiple of 8, so every `f64`
 *  column that follows is 8-byte aligned. */
export function columnsAt(stringCount: number, blobBytes: number): number {
  const head = blobAt(stringCount) + blobBytes;
  return head + ((8 - (head % 8)) % 8);
}

/** The offsets table and the blob length it ends on. `offsets[0] = 0`, entry `i` is where
 *  `strings[i]` starts, and the last is the blob length — so the slices exactly tile the blob
 *  and the table never decreases (which is what lets a reader binary-search it). */
export interface TableOffsets {
  readonly offsets: Uint32Array;
  readonly blobBytes: number;
}

/** What the measuring pass found: each entry's width, the offsets they add up to, and the blob
 *  length the offsets table ends on. The writing pass reads the widths from here rather than
 *  measuring again, so the two passes cannot disagree about a byte count. */
export interface Measured {
  readonly widths: Uint32Array;
  readonly table: TableOffsets;
}

/** One pass over the table with `width`, keeping what it found. */
export function measureBlob(
  strings: readonly string[],
  width: (value: string, index: number) => number,
): Measured {
  const widths = new Uint32Array(strings.length);
  const offsets = new Uint32Array(strings.length + 1);
  let running = 0;
  for (const [i, value] of strings.entries()) {
    offsets[i] = running;
    widths[i] = width(value, i);
    running += widths[i];
  }
  offsets[strings.length] = running;
  return { widths, table: { offsets, blobBytes: running } };
}

/** Puts the whole table's bytes into `out` at `at`, in **one** `encodeInto`, over the joined
 *  text: `measureBlob` has already measured every entry and `exactWidth` has already refused
 *  every entry that cannot be encoded, and a well-formed entry can neither open with a low
 *  surrogate nor close with a high one — so the join encodes to exactly the concatenation its
 *  entries' widths named. Both halves of that are checked: `read` is the whole joined text
 *  consumed and `written` is the measured total, so a table that measured and a table that
 *  encoded cannot disagree.
 *
 *  This is where the exact path's cost went. Placing entry by entry meant a `subarray` object
 *  per entry — 160 000 throwaway views a batch — and `docs/measurements/perf-p4g-wasm.md`
 *  measures that form at **9.76 ms** against this one's **3.80 ms** for the same 160 000
 *  entries and 1.76 MB of blob. The cost was the views, not the encoding.
 *
 *  Caveat: this allocates the joined text, a copy of the blob. It is **read**, not discarded —
 *  which is the whole difference from the copy `perf-p4f-wasm.md` removed, where a join was
 *  made, sized at the code-unit count, found too short and thrown away. */
export function placeTable(
  out: Uint8Array,
  at: number,
  strings: readonly string[],
  widths: Uint32Array,
): void {
  const blobBytes = widths.reduce((sum, width) => sum + width, 0);
  const text = joinTable(strings);
  const written = ENCODER.encodeInto(text, out.subarray(at, at + blobBytes));
  if (written.read !== text.length || written.written !== blobBytes) {
    throw new ColumnsEncoderError("string table", "encoded to a length other than it measured");
  }
}

/** Whether every code unit of every entry is one UTF-8 byte.
 *
 *  The whole table, not a prefix: the joined path sizes its buffer at `text.length` and a wide
 *  entry anywhere would overflow it. Scanning code units costs one pass and allocates nothing,
 *  where the previous arrangement — join the table, size at `text.length`, ask `encodeInto`
 *  whether that was enough — allocated a second copy of the whole blob and *threw it away*
 *  whenever the answer was no, which is every table holding an emoji. */
export function allAscii(strings: readonly string[]): boolean {
  for (const value of strings) {
    for (let i = 0; i < value.length; i += 1) {
      if (value.charCodeAt(i) > 0x7f) return false;
    }
  }
  return true;
}

/** One code unit is one byte, so an ASCII entry's width is its length. The only `width` that
 *  does not walk the string, and the reason an ASCII table's measuring pass is cheaper. */
export function asciiWidth(value: string): number {
  return value.length;
}

/** One entry's UTF-8 width, and its well-formedness, in a single walk.
 *
 *  A lone surrogate is refused here, by index, rather than measured: it is `0xD800`–`0xDFFF`,
 *  `utf8Length` would count it as a code point, and `encodeInto` would write U+FFFD in its
 *  place — so the row would read back as a *different* id, silently (D9 at the string level).
 *  Two lone surrogates in two entries join into one valid pair, which is why the check is here,
 *  per entry, and never on the joined text. */
export function exactWidth(value: string, index: number): number {
  if (!value.isWellFormed()) throw new ColumnsEncoderError(`strings[${index}]`, NOT_WELL_FORMED);
  return utf8Length(value);
}

/** One string's UTF-8 length, counted rather than encoded: a million `encode` calls would be
 *  a million throwaway arrays, and this runs before the writing one.
 *
 *  Counted in **code units**, not code points: a `for…of` over a string yields one string per
 *  code point, so it allocates a throwaway string per character — a megabyte of garbage a
 *  batch on the stream in `docs/measurements/perf-p4g-wasm.md`, for a number `charCodeAt`
 *  gives directly. The three width classes are the UTF-8 encoding's own (`< 0x80` one byte,
 *  `< 0x800` two, the rest three), with the surrogate range as the fourth: a high surrogate
 *  and the low one after it are one code point and four bytes.
 *
 *  Every input, the same number as the code-point walk it replaces. **Caveat:** the index is
 *  widened past the low half of a *whole* pair and past nothing else, so a lone high
 *  surrogate counts three and the code unit after it is still counted — three bytes is what
 *  the walk it replaced counted, and `encodeInto` refuses the row outright.
 *  `exactWidth` refuses a lone surrogate before this is reached, by index. */
export function utf8Length(value: string): number {
  let bytes = 0;
  for (let i = 0; i < value.length; i += 1) {
    const unit = value.charCodeAt(i);
    if (unit < 0x80) {
      bytes += 1;
    } else if (unit < 0x800) {
      bytes += 2;
    } else if (unit < 0xd800 || unit >= 0xe000) {
      bytes += 3;
    } else if (unit < 0xdc00 && lowSurrogate(value, i + 1)) {
      // One code point in two code units: four bytes, and the low half is not counted again.
      bytes += 4;
      i += 1;
    } else {
      bytes += 3;
    }
  }
  return bytes;
}

/** Whether code unit `i` is a low surrogate, so the high one before it is half a pair. Past the
 *  end `charCodeAt` reads `NaN`, which is in no range, so a high surrogate last is lone. */
function lowSurrogate(value: string, i: number): boolean {
  const unit = value.charCodeAt(i);
  return unit >= 0xdc00 && unit < 0xe000;
}

/** The joined text of the table, which is the whole blob in one string and the one thing
 *  {@link placeTable} encodes: a well-formed entry can neither open with a low surrogate nor
 *  close with a high one, so the join encodes to exactly the concatenation its entries' widths
 *  named. */
export function joinTable(strings: readonly string[]): string {
  return strings.join("");
}

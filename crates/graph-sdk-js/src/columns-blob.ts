// The string table's **blob**: where it starts, how wide each entry is, and the one loop that
// puts the bytes there. Split out of `columns-assemble.ts`, which was over the house's line limit
// once this file's subject — and its two paths — grew a module of its own.
//
// Two paths, and which one runs is decided by **one pass that measures nothing**:
//
//   * every code unit ASCII — the table is joined and placed by a single `encodeInto`, and each
//     entry's width is its code-unit count;
//   * anything wider — every entry is measured and placed into the slot its own width names.
//
// Both write the same bytes for the same table, which is what `test/columns-blob.test.mjs` pins.
// The split is worth a module of its own because the two cost very different amounts and the
// studio's own data — icons `🌿` and `📈` — is on the second one.
//
// **Caveat:** the ASCII test is a code-unit scan, so a string that is *well-formed but wide*
// (an emoji, an `é`) takes the exact path, and a table of only-ASCII takes the joined one. Both
// produce the same document; only the cost differs.

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

/** Puts every entry's bytes into `out` at the slot its measured width names, starting at `at`.
 *
 *  `out.subarray(cursor, end)` is the slot, and `encodeInto` stops at `end` — so an entry cannot
 *  spill into its neighbour's bytes even if it were to encode wider than it measured, which is
 *  what the `written` check turns into a refusal rather than a corrupt document. */
export function placeBlob(out: Uint8Array, at: number, strings: readonly string[], widths: Uint32Array): void {
  let cursor = at;
  for (const [i, value] of strings.entries()) {
    const width = widths[i] ?? 0;
    const written = ENCODER.encodeInto(value, out.subarray(cursor, cursor + width)).written;
    if (written !== width) {
      throw new ColumnsEncoderError("string table", "encoded to a length other than it measured");
    }
    cursor += width;
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
 *  does not walk the string, and the reason the joined path is one `encodeInto` for the lot. */
export function asciiWidth(value: string): number {
  return value.length;
}

/** {@link placeBlob} for a table {@link allAscii} cleared: one `encodeInto` over the joined text,
 *  placed at `at`. The widths are not read here — for an ASCII table `text.length` *is* the blob
 *  length, which is the whole reason this path exists — so they are not taken. */
export function placeJoined(out: Uint8Array, at: number, strings: readonly string[]): void {
  const text = joinTable(strings);
  const written = ENCODER.encodeInto(text, out.subarray(at, at + text.length));
  if (written.read !== text.length || written.written !== text.length) {
    throw new ColumnsEncoderError("string table", "encoded to a length other than it measured");
  }
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
 *  a million throwaway arrays, and this runs before the writing one. */
export function utf8Length(value: string): number {
  let bytes = 0;
  for (const chunk of value) {
    const point = chunk.codePointAt(0) ?? 0;
    bytes += point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x1_0000 ? 3 : 4;
  }
  return bytes;
}

/** The joined text of an all-ASCII table, which is the whole blob in one string. The exact path
 *  never calls this: joining a table it is about to place entry by entry would be a whole extra
 *  copy of the blob, allocated and discarded. */
export function joinTable(strings: readonly string[]): string {
  return strings.join("");
}

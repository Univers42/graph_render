// The bytes of a columnar ingest document, from rows that are *already* columns.
//
// `columns.ts` is the general producer: it walks a document of records and interns every
// string through a `Map`. At a million nodes that Map was 23 million probes over roughly seven
// million distinct strings — `docs/measurements/perf-open-columns.md` measured it at 6.4 s,
// slower than `JSON.stringify`. A producer that already holds the answer as columns does not
// need it: the contract says two table entries may hold the same bytes, and it says endpoints
// are node **row** numbers. So this module takes the table as a list and the cells as arrays,
// appends nothing, looks nothing up, and writes the buffer.
//
// Two things are still per string: the offsets, and the blob. Both collapse when the table is
// ASCII, which is the case the studio's generator produces and the case the fast path is for —
// then `text.length` is the blob length and `encodeInto` confirms it in one call, so no string
// is ever measured or encoded on its own.
//
// **Caveat:** the table is not deduped. The blob carries a repeat for every repeated value and
// the decoder's arena interns it back down by content, so the cost is bytes in transit and
// nothing downstream.
//
// A batch (`GMX1`) is the same sections, the same order and the same arithmetic as a document
// (`GMC1`), which is why this module writes both: the one word that says which reader the bytes
// are for is the magic, and it travels with the rows (`ColumnRows.magic`). What an endpoint
// *means* is not this module's business — it copies the cells it is handed, so the dense rows of
// a document and the string indices of a batch are both just `u32`s by the time they arrive.

import { GraphMotorError } from "./errors.ts";
import {
  type TableOffsets,
  allAscii,
  asciiWidth,
  blobAt,
  columnsAt,
  exactWidth,
  measureBlob,
  placeTable,
} from "./columns-blob.ts";

export { ColumnsEncoderError, NOT_WELL_FORMED } from "./columns-blob.ts";
export { utf8Length } from "./columns-blob.ts";

/** `u32::MAX` in an optional column: "this field is absent". */
export const ABSENT = 0xffff_ffff;
/** `0x31434D47`: `"GMC1"` as a little-endian `u32` — a whole document.
 *  **Caveat:** a copy of the Rust `MAGIC`; the two are pinned by the decoder's tests. */
export const DOCUMENT_MAGIC = 0x3143_4d47;
/** `0x31584D47`: `"GMX1"` as a little-endian `u32` — a batch, which `gm_graph_extend_columns`
 *  reads. A distinct magic and not a version or a flag, so each reader refuses the other's bytes
 *  by name (`docs/decisions/extend-columns.md`, "The magic"). */
export const BATCH_MAGIC = 0x3158_4d47;
/** The version this encoder writes, and the only one the decoder speaks. */
const VERSION = 1;
/** The header is eight `u32` words. */
const HEADER_BYTES = 32;
/** The number of `u32` columns on each side. */
export const NODE_COLUMNS = 8;
export const EDGE_COLUMNS = 8;

/**
 * Ponytail: the format is little-endian and every JavaScript engine in use is little-endian,
 * so the `f64` and `u32` columns are copied with one typed-array `set` per column instead of
 * one `DataView` call per cell — at a million nodes that is the difference between sixteen
 * `set` calls and sixteen million. Failing input: a big-endian host, of which no JavaScript
 * engine is known. Direction: when this reads false the writers take the `DataView` path
 * instead, which produces the same bytes. Escape hatch: {@link assembleColumnsAs} takes the
 * byte order as an argument, so a test on a little-endian host runs the fallback too and
 * compares its bytes with the fast path's (`test/columns-assemble.test.mjs`).
 */
const LITTLE_ENDIAN = new Uint8Array(new Uint16Array([1]).buffer)[0] === 1;

/** A document as columns: the string table, then every field already a column. */
export interface ColumnRows {
  /** The string table in index order. Duplicates are allowed: the contract does not dedupe. */
  readonly strings: readonly string[];
  /** The 8 node `u32` columns, column-major: column c of row r is at `c * nodeCount + r`. */
  readonly nodeCells: Uint32Array;
  /** The 8 edge `u32` columns, column-major. Endpoints are node rows under `GMC1` and string
   *  entries naming node ids under `GMX1`; which one is written is {@link ColumnRows.magic}'s
   *  answer, and the encoder is what produces the matching cells. */
  readonly edgeCells: Uint32Array;
  readonly weights: Float64Array;
  readonly versions: Float64Array;
  readonly strengths: Float64Array;
  /** Which reader these bytes are for: {@link DOCUMENT_MAGIC} (the default) or
   *  {@link BATCH_MAGIC}. It travels with the rows because it is a property of the document
   *  being written, not of the writer: the sections are the same either way and only this one
   *  header word differs, so every producer that writes whole documents names nothing. */
  readonly magic?: number;
}

/** Assembles `rows` into the binary document `gm_build_columns` reads, or throws.
 *
 * # Panics
 *
 * The same refusals `encodeColumns` does not make: a non-finite `weight`, `version` or
 * `strength`, or a cell of `u32::MAX` in a required column. The decoder refuses both. */
export function assembleColumns(rows: ColumnRows): Uint8Array {
  return assembleColumnsAs(rows, LITTLE_ENDIAN);
}

/** {@link assembleColumns} with the host byte order given rather than probed. Not re-exported
 *  from the package: it exists so a test can run the `DataView` writers on a little-endian
 *  host.
 *
 *  Two **measuring** paths, and which one runs is decided by **one pass that measures nothing**:
 *
 *  - every code unit ASCII — the offsets are code-unit counts;
 *  - anything wider — every entry is measured with {@link utf8Length}, which is also where a
 *    lone surrogate is refused.
 *
 *  Both are then placed by one `encodeInto` over the joined text. Both write the same bytes for
 *  the same table, which is what `test/columns-blob.test.mjs` pins. */
export function assembleColumnsAs(rows: ColumnRows, littleEndian: boolean): Uint8Array {
  checkLengths(rows);
  // One measuring pass, then one writing pass, and the second is checked against the first's
  // total. `columns-blob.ts` owns both, and with them the choice between the two width functions
  // — see there for why the studio's data takes the second.
  const ascii = allAscii(rows.strings);
  const { widths, table } = measureBlob(rows.strings, ascii ? asciiWidth : exactWidth);
  const out = new Uint8Array(bufferLength(rows, table.blobBytes));
  placeTable(out, blobAt(rows.strings.length), rows.strings, widths);
  finish(out, rows, table, littleEndian);
  return out;
}

/** The whole buffer length. Never grown: the decoder refuses a buffer whose declared sections
 *  do not sum to its exact length, so the size has to be known before a byte is written. */
function bufferLength(rows: ColumnRows, blobBytes: number): number {
  const ints = (NODE_COLUMNS * rows.weights.length + EDGE_COLUMNS * rows.strengths.length) * 4;
  const floats = rows.weights.length + rows.versions.length + rows.strengths.length;
  return columnsAt(rows.strings.length, blobBytes) + 8 * floats + ints;
}

/** A column array of the wrong length is a producer bug. Saying which array and how long it
 *  was is the difference between a one-line fix and a bisect. */
function checkColumn(name: string, held: number, want: number): void {
  if (held !== want) {
    throw new GraphMotorError(
      `\`${name}\` holds ${held} cells, but this document needs ${want}`,
    );
  }
}

function checkLengths(rows: ColumnRows): void {
  const nodes = rows.weights.length;
  const edges = rows.strengths.length;
  checkColumn("versions", rows.versions.length, nodes);
  checkColumn("nodeCells", rows.nodeCells.length, NODE_COLUMNS * nodes);
  checkColumn("edgeCells", rows.edgeCells.length, EDGE_COLUMNS * edges);
}

/** The eight `u32` words: the magic, the counts, the blob length, and two reserved zeros. The
 *  magic is the only word that says which reader these bytes are for. */
function writeHeader(out: Uint8Array, rows: ColumnRows, blobBytes: number): void {
  const header = [
    rows.magic ?? DOCUMENT_MAGIC, VERSION, rows.weights.length, rows.strengths.length,
    rows.strings.length, blobBytes, 0, 0,
  ];
  const view = new DataView(out.buffer);
  header.forEach((word, i) => view.setUint32(4 * i, word, true));
}

/** The three `f64` columns, in contract order. `set` writes the IEEE-754 bits directly, so
 *  `-0` and a subnormal round-trip as themselves — both ordinary finite doubles, and the JSON
 *  reader accepts them too. The offsets are multiples of 8 and `out` is fresh, so the views
 *  are always in bounds and never straddle. */
function writeFloats(
  out: Uint8Array,
  at: number,
  columns: readonly Float64Array[],
  littleEndian: boolean,
): void {
  let cursor = at;
  const view = littleEndian ? null : new DataView(out.buffer);
  for (const column of columns) {
    if (view === null) {
      new Float64Array(out.buffer, cursor, column.length).set(column);
    } else {
      column.forEach((value, i) => view.setFloat64(cursor + 8 * i, value, true));
    }
    cursor += 8 * column.length;
  }
}

/** `u32` columns, one after another: the offsets table, and the sixteen cell columns in
 *  contract order — eight node columns then eight edge columns, each column's rows contiguous,
 *  which is the order {@link ColumnRows} hands them over, so this is a copy per column. */
function writeInts(
  out: Uint8Array,
  at: number,
  columns: readonly Uint32Array[],
  littleEndian: boolean,
): void {
  let cursor = at;
  const view = littleEndian ? null : new DataView(out.buffer);
  for (const column of columns) {
    if (view === null) {
      new Uint32Array(out.buffer, cursor, column.length).set(column);
    } else {
      column.forEach((value, i) => view.setUint32(cursor + 4 * i, value, true));
    }
    cursor += 4 * column.length;
  }
}

/** Everything but the blob, which the caller has already placed. */
function finish(
  out: Uint8Array,
  rows: ColumnRows,
  table: TableOffsets,
  littleEndian: boolean,
): void {
  writeHeader(out, rows, table.blobBytes);
  // The offsets table sits between the header and the blob: `string_count + 1` words at
  // `HEADER_BYTES`, so the blob begins at `blobAt(stringCount)`.
  writeInts(out, HEADER_BYTES, [table.offsets], littleEndian);
  const floats = columnsAt(rows.strings.length, table.blobBytes);
  const ints = floats + 8 * (rows.weights.length + rows.versions.length + rows.strengths.length);
  writeFloats(out, floats, [rows.weights, rows.versions, rows.strengths], littleEndian);
  writeInts(out, ints, [rows.nodeCells, rows.edgeCells], littleEndian);
}


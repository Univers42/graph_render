// The columnar ingest encoder: a provisional ingest document to the little-endian binary
// `gm_build_columns` reads (`docs/contract/ingest-columns.md`,
// `docs/decisions/ingest-columns.md`).
//
// Two properties are the reason this exists as its own file rather than as a few lines in
// `index.ts`, which is already at the house's line limit:
//
//   * one `Uint8Array`, sized before a byte is written. The decoder refuses a buffer whose
//     declared sections do not sum to its exact length, so the size has to be *known* —
//     which means two passes over the document: one to intern and measure, one to write.
//   * `TextEncoder.encodeInto` into that one buffer. `TextEncoder.encode` would allocate a
//     second array per string and copy it in; `encodeInto` writes where it is told to.
//
// Nothing here allocates per row. The 1M measurement in
// `docs/measurements/perf-open-columns.md` is where that was learned: a `Uint32Array` per
// node and per edge is four million short-lived typed arrays, and the encoder was slower
// than `JSON.stringify` because of them. The cells go into one array instead.
//
// **Caveat:** the string table is deduped through a `Map`, whose iteration order is
// insertion order, so the bytes are a pure function of the document. The offsets the decoder
// reads are therefore stable, but the *indices* are not an API: nothing outside this file
// may depend on which index a given value got.

import { GraphMotorError } from "./errors.ts";

/** One node, in the provisional ingest shape (`crates/graph-wasm/src/ingest/record.rs`). */
export interface ColumnsNode {
  id: string;
  kind: string;
  database_id: string | null;
  source: string;
  label: string;
  group: string | null;
  weight: number;
  version: number;
  has_note: boolean;
  icon: string | null;
}

/** One edge, in the provisional ingest shape. */
export interface ColumnsEdge {
  id: string;
  source: string;
  target: string;
  kind: string;
  label: string;
  strength: number;
  directed: boolean;
  record_id: string | null;
  child_first: boolean;
}

/** A whole ingest document, the same shape `gm_build` reads as JSON. */
export interface ColumnsDocument {
  version: number;
  nodes: ColumnsNode[];
  edges: ColumnsEdge[];
}

/** A string that cannot be encoded, with the field that holds it. */
export class ColumnsEncoderError extends GraphMotorError {
  /** The dotted field path, e.g. `nodes[7].icon` — a caller can branch on this. */
  readonly field: string;

  constructor(field: string) {
    super(
      `\`${field}\` is not well-formed: a lone surrogate cannot be encoded as UTF-8 and ` +
        `would read back as a different id (U+FFFD), so the document is refused instead`,
    );
    this.field = field;
  }
}

/** `u32::MAX` in an optional column: "this field is absent". */
const ABSENT = 0xffff_ffff;
/** `0x31434D47`: `"GMC1"` as a little-endian `u32`.
 *  **Caveat:** a copy of the Rust `MAGIC`; the two are pinned by the decoder's tests. */
const MAGIC = 0x3143_4d47;
/** The version this encoder writes, and the only one the decoder speaks. */
const VERSION = 1;
/** The header is eight `u32` words. */
const HEADER_BYTES = 32;
/** The number of `u32` columns on each side. */
const NODE_COLUMNS = 8;
const EDGE_COLUMNS = 8;
/** One encoder for the writing pass; {@link utf8Length} counts the measuring pass. */
const ENCODER = new TextEncoder();

/** Encodes `doc` as the columnar document, or throws [`ColumnsEncoderError`].
 *
 * # Panics
 *
 * A value the contract forbids but this encoder does not check: a non-finite `weight`,
 * `version` or `strength`, or a boolean field that is not a boolean. The decoder refuses all
 * of them with `ColumnsInvalid`; this side refuses only what it can name for the caller,
 * which is the string that would silently come back as something else. */
export function encodeColumns(doc: ColumnsDocument): Uint8Array {
  const table = new Table();
  const rows = new Map<string, number>();
  const cells = new Uint32Array(
    NODE_COLUMNS * doc.nodes.length + EDGE_COLUMNS * doc.edges.length,
  );
  doc.nodes.forEach((node, row) => {
    rows.set(node.id, row);
    table.nodeCells(node, row, cells, 0);
  });
  const edgeBase = NODE_COLUMNS * doc.nodes.length;
  doc.edges.forEach((edge, row) => table.edgeCells(edge, row, cells, edgeBase, rows));
  const floats = [
    doc.nodes.map((node) => node.weight),
    doc.nodes.map((node) => node.version),
    doc.edges.map((edge) => edge.strength),
  ];
  return assemble(table, cells, floats, doc.nodes.length, doc.edges.length);
}

/** The string table, interned in first-seen order. A `Map`, never a hash map iterated for
 *  output: an entry's index is its position in `strings`, and `Map` preserves insertion
 *  order, so the same document always produces the same bytes on every engine (D4). */
class Table {
  readonly strings: string[] = [];
  readonly offsets: number[] = [0];
  readonly index = new Map<string, number>();
  /** Running blob length, so the measuring pass never reads past the array end. */
  private bytes = 0;

  intern(field: string, value: string): number {
    const at = this.index.get(value);
    if (at !== undefined) return at;
    // `isWellFormed` is the check: `encodeInto` would otherwise replace a lone surrogate
    // with U+FFFD, and the row would read back as a different id (D9 at the string level).
    if (!value.isWellFormed()) throw new ColumnsEncoderError(field);
    const next = this.strings.length;
    this.strings.push(value);
    this.index.set(value, next);
    this.bytes += utf8Length(value);
    this.offsets.push(this.bytes);
    return next;
  }

  optional(field: string, value: string | null): number {
    return value === null ? ABSENT : this.intern(field, value);
  }

  /** The eight node columns of row `row`, into `cells` at `base + row * NODE_COLUMNS`. */
  nodeCells(node: ColumnsNode, row: number, cells: Uint32Array, base: number): void {
    const at = base + row * NODE_COLUMNS;
    cells[at] = this.intern(`nodes[${row}].id`, node.id);
    cells[at + 1] = this.intern(`nodes[${row}].kind`, node.kind);
    cells[at + 2] = this.optional(`nodes[${row}].database_id`, node.database_id);
    cells[at + 3] = this.intern(`nodes[${row}].source`, node.source);
    cells[at + 4] = this.intern(`nodes[${row}].label`, node.label);
    cells[at + 5] = this.optional(`nodes[${row}].group`, node.group);
    cells[at + 6] = this.optional(`nodes[${row}].icon`, node.icon);
    cells[at + 7] = node.has_note ? 1 : 0;
  }

  /** The eight edge columns of row `row`. An endpoint that names no node is refused here,
   *  naming the field: the decoder refuses it too, but one lookup turning that into a stack
   *  a caller can read is worth the branch. */
  edgeCells(
    edge: ColumnsEdge,
    row: number,
    cells: Uint32Array,
    base: number,
    rows: Map<string, number>,
  ): void {
    const source = rows.get(edge.source);
    const target = rows.get(edge.target);
    if (source === undefined || target === undefined) {
      throw new ColumnsEncoderError(`edges[${row}].${source === undefined ? "source" : "target"}`);
    }
    const at = base + row * EDGE_COLUMNS;
    cells[at] = this.intern(`edges[${row}].id`, edge.id);
    cells[at + 1] = source;
    cells[at + 2] = target;
    cells[at + 3] = this.intern(`edges[${row}].kind`, edge.kind);
    cells[at + 4] = this.intern(`edges[${row}].label`, edge.label);
    cells[at + 5] = this.optional(`edges[${row}].record_id`, edge.record_id);
    cells[at + 6] = edge.directed ? 1 : 0;
    cells[at + 7] = edge.child_first ? 1 : 0;
  }

  blobBytes(): number {
    return this.bytes;
  }
}

/** The bytes. The size is computed from the counts and the measured blob, never grown: the
 *  decoder refuses a buffer whose declared sections do not sum to its exact length. */
function assemble(
  table: Table,
  cells: Uint32Array,
  floats: number[][],
  nodes: number,
  edges: number,
): Uint8Array {
  const blob = table.blobBytes();
  const head = HEADER_BYTES + 4 * (table.strings.length + 1) + blob;
  const columns = head + ((8 - (head % 8)) % 8);
  const wide = floats.reduce((sum, column) => sum + column.length, 0);
  const out = new Uint8Array(columns + 8 * wide + 4 * cells.length);
  const view = new DataView(out.buffer);
  const header = [MAGIC, VERSION, nodes, edges, table.strings.length, blob, 0, 0];
  header.forEach((word, i) => view.setUint32(4 * i, word, true));
  table.offsets.forEach((offset, i) => view.setUint32(HEADER_BYTES + 4 * i, offset, true));
  writeBlob(table, out, HEADER_BYTES + 4 * (table.strings.length + 1));
  writeFloats(view, columns, floats);
  writeInts(view, columns + 8 * wide, cells, nodes);
  return out;
}

/** The blob, in the one buffer, through `encodeInto` — a second `TextEncoder.encode` would
 *  allocate an array per string and copy it in. */
function writeBlob(table: Table, out: Uint8Array, at: number): void {
  let cursor = at;
  for (const value of table.strings) {
    const written = ENCODER.encodeInto(value, out.subarray(cursor)).written;
    if (written !== utf8Length(value)) throw new ColumnsEncoderError("string table");
    cursor += written;
  }
}

/** The three `f64` columns. `setFloat64` writes the IEEE-754 bits directly, so `-0` and a
 *  subnormal round-trip as themselves — both are ordinary finite doubles, and the decoder
 *  accepts them because the JSON reader does. */
function writeFloats(view: DataView, at: number, columns: number[][]): void {
  let cursor = at;
  for (const column of columns) {
    for (const value of column) {
      view.setFloat64(cursor, value, true);
      cursor += 8;
    }
  }
}

/**
 * The sixteen `u32` columns, in contract order: eight node columns then eight edge
 * columns, each column's rows contiguous. `cells` is held row-major because that is how a
 * row is filled in as the document is walked; the buffer is column-major because that is
 * what the contract specifies and what the decoder reads one row of at a time.
 */
function writeInts(view: DataView, at: number, cells: Uint32Array, nodes: number): void {
  let cursor = at;
  for (const [base, width, count] of [
    [0, NODE_COLUMNS, NODE_COLUMNS * nodes],
    [NODE_COLUMNS * nodes, EDGE_COLUMNS, cells.length - NODE_COLUMNS * nodes],
  ] as [number, number, number][]) {
    for (let column = 0; column < width; column += 1) {
      for (let row = 0; row < count / width; row += 1) {
        view.setUint32(cursor, cells[base + row * width + column] ?? ABSENT, true);
        cursor += 4;
      }
    }
  }
}

/** One string's UTF-8 length, counted rather than encoded: a million `encode` calls would be
 *  a million throwaway arrays, and the measuring pass runs before the writing one. */
function utf8Length(value: string): number {
  let bytes = 0;
  for (const chunk of value) {
    const point = chunk.codePointAt(0) ?? 0;
    bytes += point < 0x80 ? 1 : point < 0x800 ? 2 : point < 0x1_0000 ? 3 : 4;
  }
  return bytes;
}
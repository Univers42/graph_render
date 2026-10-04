// The columnar ingest encoder: a provisional ingest document to the little-endian binary
// `gm_build_columns` reads (`docs/contract/ingest-columns.md`,
// `docs/decisions/ingest-columns.md`).
//
// Two properties are the reason this exists as its own file rather than as a few lines in
// `index.ts`, which is already at the house's line limit:
//
//   * one `Uint8Array`, sized before a byte is written. The decoder refuses a buffer whose
//     declared sections do not sum to its exact length, so the size has to be *known* —
//     which means two passes over the document: one to intern, one to write.
//   * `TextEncoder.encodeInto` into that one buffer. `TextEncoder.encode` would allocate a
//     second array per string and copy it in; `encodeInto` writes where it is told to.
//
// The bytes themselves are `columns-assemble.ts`'s: this file is the *general* producer, which
// walks records and interns every string through a `Map` and resolves every endpoint by id. A
// producer that already holds columns calls `assembleColumns` directly and skips both.
//
// **Caveat:** the string table is deduped through a `Map`, whose iteration order is
// insertion order, so the bytes are a pure function of the document. The offsets the decoder
// reads are therefore stable, but the *indices* are not an API: nothing outside this file
// may depend on which index a given value got.

import {
  ABSENT,
  ColumnsEncoderError,
  EDGE_COLUMNS,
  NODE_COLUMNS,
  NOT_WELL_FORMED,
  assembleColumns,
} from "./columns-assemble.ts";

export { ColumnsEncoderError } from "./columns-assemble.ts";
export type { ColumnRows } from "./columns-assemble.ts";

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
  const nodeCount = doc.nodes.length;
  const edgeCount = doc.edges.length;
  // Column-major from the start: the contract stores one column after another, and the
  // assembler copies a column per `set`. Holding this row-major and transposing on the way out
  // is what `docs/measurements/perf-open-columns.md` measured — a `Uint32Array` per node and
  // per edge is four million short-lived typed arrays at 1M, and the encoder lost to
  // `JSON.stringify` because of them.
  const nodeCells = new Uint32Array(NODE_COLUMNS * nodeCount);
  const edgeCells = new Uint32Array(EDGE_COLUMNS * edgeCount);
  const rows = new Map<string, number>();
  doc.nodes.forEach((node, row) => {
    rows.set(node.id, row);
    table.nodeCells(node, row, nodeCells, nodeCount);
  });
  doc.edges.forEach((edge, row) => table.edgeCells(edge, row, edgeCells, rows));
  return assembleColumns({
    strings: table.strings,
    nodeCells,
    edgeCells,
    weights: Float64Array.from(doc.nodes, (node) => node.weight),
    versions: Float64Array.from(doc.nodes, (node) => node.version),
    strengths: Float64Array.from(doc.edges, (edge) => edge.strength),
  });
}

/** The string table, interned in first-seen order. A `Map`, never a hash map iterated for
 *  output: an entry's index is its position in `strings`, and `Map` preserves insertion
 *  order, so the same document always produces the same bytes on every engine (D4). */
class Table {
  readonly strings: string[] = [];
  readonly index = new Map<string, number>();

  intern(field: string, value: string): number {
    const at = this.index.get(value);
    if (at !== undefined) return at;
    // `isWellFormed` is the check: `encodeInto` would otherwise replace a lone surrogate
    // with U+FFFD, and the row would read back as a different id (D9 at the string level).
    if (!value.isWellFormed()) throw new ColumnsEncoderError(field, NOT_WELL_FORMED);
    const next = this.strings.length;
    this.strings.push(value);
    this.index.set(value, next);
    return next;
  }

  optional(field: string, value: string | null): number {
    return value === null ? ABSENT : this.intern(field, value);
  }

  /** The eight node columns of row `row`, at `column * count + row`. */
  nodeCells(node: ColumnsNode, row: number, cells: Uint32Array, count: number): void {
    cells[row] = this.intern(`nodes[${row}].id`, node.id);
    cells[count + row] = this.intern(`nodes[${row}].kind`, node.kind);
    cells[2 * count + row] = this.optional(`nodes[${row}].database_id`, node.database_id);
    cells[3 * count + row] = this.intern(`nodes[${row}].source`, node.source);
    cells[4 * count + row] = this.intern(`nodes[${row}].label`, node.label);
    cells[5 * count + row] = this.optional(`nodes[${row}].group`, node.group);
    cells[6 * count + row] = this.optional(`nodes[${row}].icon`, node.icon);
    cells[7 * count + row] = node.has_note ? 1 : 0;
  }

  /** The eight edge columns of row `row`, at `column * count + row` where `count` is
   *  `cells.length / EDGE_COLUMNS`. An endpoint that names no node is refused here, naming the
   *  field: the decoder refuses it too, but one lookup turning that into a stack a caller can
   *  read is worth the branch. */
  edgeCells(edge: ColumnsEdge, row: number, cells: Uint32Array, rows: Map<string, number>): void {
    const count = cells.length / EDGE_COLUMNS;
    const source = rows.get(edge.source);
    const target = rows.get(edge.target);
    if (source === undefined || target === undefined) {
      const end = source === undefined ? "source" : "target";
      throw new ColumnsEncoderError(`edges[${row}].${end}`, "names no node");
    }
    cells[row] = this.intern(`edges[${row}].id`, edge.id);
    cells[count + row] = source;
    cells[2 * count + row] = target;
    cells[3 * count + row] = this.intern(`edges[${row}].kind`, edge.kind);
    cells[4 * count + row] = this.intern(`edges[${row}].label`, edge.label);
    cells[5 * count + row] = this.optional(`edges[${row}].record_id`, edge.record_id);
    cells[6 * count + row] = edge.directed ? 1 : 0;
    cells[7 * count + row] = edge.child_first ? 1 : 0;
  }
}

// `encodeBatch`: a `GraphBatch` as the `GMX1` binary document `gm_graph_extend_columns` reads
// (`docs/contract/ingest-columns.md`, `docs/decisions/extend-columns.md`).
//
// It is `encodeColumns` over the same two passes and the same `Table`, and the difference is one
// method: a batch's edge endpoint is a **string index naming a node id**, so it is interned on
// demand rather than resolved to a row of this document — an endpoint may name a node the graph
// already holds, and a dense row never crosses the wire. Everything else (the sized buffer, the
// interning order, the lone-surrogate refusal by field name, the section arithmetic) is shared,
// which is why this is a module of twenty lines and not a second encoder.
//
// The magic is the assembler's one word; the sections are identical, so `assembleColumns` writes
// both and neither encoder has a second copy of the layout.

import { Table } from "./columns.ts";
import { BATCH_MAGIC, EDGE_COLUMNS, NODE_COLUMNS, assembleColumns } from "./columns-assemble.ts";
import type { GraphBatch } from "./extend.ts";

/** Encodes `batch` as the columnar batch document, or throws
 *  [`ColumnsEncoderError`](./columns.ts). One `Uint8Array`, sized before a byte is written, for
 *  the same reason a document's is: the decoder refuses a buffer whose declared sections do not
 *  sum to its exact length.
 *
 *  # Panics
 *
 *  The same values {@link encodeColumns} does not check: a non-finite `weight`, `version` or
 *  `strength`, or a boolean field that is not a boolean. The decoder refuses all of them with
 *  `ColumnsInvalid`; this side refuses only what it can name for the caller.
 *
 *  **Caveat:** the string table is deduped through `Table`'s `Map`, whose iteration order is
 *  insertion order, so the bytes are a pure function of the batch — but the *indices* are not an
 *  API, exactly as in a document. */
export function encodeBatch(batch: GraphBatch): Uint8Array {
  const table = new Table();
  const nodes = batch.nodes.length;
  const edges = batch.edges.length;
  const nodeCells = new Uint32Array(NODE_COLUMNS * nodes);
  const edgeCells = new Uint32Array(EDGE_COLUMNS * edges);
  for (const [row, node] of batch.nodes.entries()) table.nodeCells(node, row, nodeCells, nodes);
  for (const [row, edge] of batch.edges.entries()) table.edgeCellsByName(edge, row, edgeCells);
  return assembleColumns({
    strings: table.strings,
    nodeCells,
    edgeCells,
    weights: column(batch.nodes, "weight"),
    versions: column(batch.nodes, "version"),
    strengths: column(batch.edges, "strength"),
    magic: BATCH_MAGIC,
  });
}

/** One `f64` column, read straight into its own `Float64Array`.
 *
 *  `Float64Array.from(nodes, (node) => node.weight)` allocates an intermediate JavaScript array of
 *  every value and then copies it into the typed array — at 10 000 nodes that is three such arrays
 *  a batch, and `docs/measurements/perf-p4f-wasm.md` measured `encodeBatch`'s own frame in the
 *  wasm32 `extend` profile. The values are already numbers and already in order, so they are
 *  written where they belong and nothing else is allocated. */
function column<K extends string>(rows: readonly Readonly<Record<K, number>>[], field: K): Float64Array {
  const out = new Float64Array(rows.length);
  for (const [row, item] of rows.entries()) out[row] = item[field];
  return out;
}

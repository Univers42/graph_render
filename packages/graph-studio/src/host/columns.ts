/**
 * What a host's columnar document is checked against, before any of it is dispatched
 * (`docs/contract/ingest-columns.md`). The verb itself is in `api.ts`; this is the one place
 * that says what a `ColumnRowsLike` has to be.
 *
 * WHY here and not in the worker: the studio never saw the rows if this refuses, so nothing set
 * `state.error` and nothing would be sent — a `TypeError` at the call boundary, the same rule
 * `loadGraph` follows for a value that is not a document (`api.ts`, `documentText`). Every refusal
 * names its field, because a host that handed over six arrays needs to be told which one.
 */
import type { ColumnRowsLike } from "../source/synthetic-columns.ts";
import { EDGE_COLUMNS, NODE_COLUMNS } from "../source/synthetic-columns.ts";

/** The `u32` columns, by the class `ColumnRowsLike` names for them. */
const CELLS = ["nodeCells", "edgeCells"] as const;
/** The `f64` columns: one per node for the first, one per edge for the last. */
const FLOATS = ["weights", "versions", "strengths"] as const;

/** The refusal every check raises: a `TypeError` naming the field, and nothing else. */
function refuse(field: string, why: string): never {
  throw new TypeError(`loadColumns: \`${field}\` ${why}`);
}

function typed<K extends "nodeCells" | "edgeCells" | "weights" | "versions" | "strengths">(
  rows: Record<string, unknown>,
  field: K,
  ctor: Uint32ArrayConstructor | Float64ArrayConstructor,
): void {
  if (!(rows[field] instanceof ctor)) refuse(field, `must be a ${ctor.name}, not ${describe(rows[field])}`);
}

function describe(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "an array";
  return `a ${typeof value}`;
}

/** The node count the columns name, or a refusal naming the field that disagrees. */
function nodeCount(cells: Uint32Array): number {
  if (cells.length % NODE_COLUMNS !== 0) refuse("nodeCells", `holds ${cells.length} cells, not ${NODE_COLUMNS} per node`);
  return cells.length / NODE_COLUMNS;
}

function edgeCount(cells: Uint32Array): number {
  if (cells.length % EDGE_COLUMNS !== 0) refuse("edgeCells", `holds ${cells.length} cells, not ${EDGE_COLUMNS} per edge`);
  return cells.length / EDGE_COLUMNS;
}

/** One `f64` per node, one per edge: `node weight`, `node version`, `edge strength`
 *  (`docs/contract/ingest-columns.md:19-21`), against the node rows of the `u32` columns
 *  (`:22-29`). A document whose counts disagree is refused by the motor, but only after the bytes
 *  are written, and one that disagrees here would open blank instead. */
function counts(rows: Record<string, unknown>, nodes: number, edges: number): void {
  const weights = rows["weights"] as Float64Array;
  const versions = rows["versions"] as Float64Array;
  const strengths = rows["strengths"] as Float64Array;
  if (weights.length !== nodes) refuse("weights", `holds ${weights.length} values for ${nodes} node rows`);
  if (versions.length !== nodes) refuse("versions", `holds ${versions.length} values for ${nodes} node rows`);
  if (strengths.length !== edges) refuse("strengths", `holds ${strengths.length} values for ${edges} edge rows`);
}

/**
 * The rows a host handed over, as the shape the worker's assembler takes, or a `TypeError` naming
 * the first field that is not what `ColumnRowsLike` says it is. A guard, not a cast: the host's
 * page is outside the element and its arrays are outside every type in this tree.
 */
export function columnRows(given: unknown): ColumnRowsLike {
  if (typeof given !== "object" || given === null) refuse("rows", "must be the columns, an object");
  const rows = given as Record<string, unknown>;
  if (!Array.isArray(rows["strings"])) refuse("strings", "must be the table, an array of strings");
  for (const field of CELLS) typed(rows, field, Uint32Array);
  for (const field of FLOATS) typed(rows, field, Float64Array);
  const nodes = nodeCount(rows["nodeCells"] as Uint32Array);
  const edges = edgeCount(rows["edgeCells"] as Uint32Array);
  counts(rows, nodes, edges);
  return given as ColumnRowsLike;
}
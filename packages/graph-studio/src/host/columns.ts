/**
 * What a host's columnar document is checked against, before any of it is dispatched
 * (`docs/contract/ingest-columns.md`). The verb itself is in `api.ts`; this is the one place
 * that says what a `ColumnRowsLike` has to be.
 *
 * WHY here and not in the worker: the studio never saw the rows if this refuses, so nothing set
 * `state.error` and nothing would be sent — a `TypeError` at the call boundary, the same rule
 * `loadGraph` follows for a value that is not a document (`api.ts`, `documentText`). Every refusal
 * names its field, because a host that handed over six arrays needs to be told which one.
 *
 * No type assertions anywhere: the rows come from a host page and are outside every type in this
 * tree, so each one is narrowed by `instanceof` and the document is rebuilt field by field.
 */
import type { ColumnRowsLike } from "../source/synthetic-columns.ts";
import { EDGE_COLUMNS, NODE_COLUMNS } from "../source/synthetic-columns.ts";

/** The refusal every check raises: a `TypeError` naming the field, and nothing else. */
function refuse(field: string, why: string): never {
  throw new TypeError(`loadColumns: \`${field}\` ${why}`);
}

/** What the value is, in a word: a refusal that cannot say is not one a host can act on. */
function describe(value: unknown): string {
  if (value === null) return "null";
  if (Array.isArray(value)) return "an array";
  return `a ${typeof value}`;
}

/** A guard, not a cast: the host's object is not `Record<string, unknown>` until it says so. */
function isRecord(given: unknown): given is Record<string, unknown> {
  return typeof given === "object" && given !== null && !Array.isArray(given);
}

function cells(rows: Record<string, unknown>, field: "nodeCells" | "edgeCells"): Uint32Array {
  const value = rows[field];
  if (!(value instanceof Uint32Array)) refuse(field, `must be a Uint32Array, not ${describe(value)}`);
  return value;
}

function column(rows: Record<string, unknown>, field: "weights" | "versions" | "strengths"): Float64Array {
  const value = rows[field];
  if (!(value instanceof Float64Array)) refuse(field, `must be a Float64Array, not ${describe(value)}`);
  return value;
}

/** The table, every entry a string: a host that wrote a number into it has a bug of its own.
 *  Collected into a new array rather than filtered twice, so one pass and no `as`. */
function table(rows: Record<string, unknown>): readonly string[] {
  const value = rows["strings"];
  if (!Array.isArray(value)) refuse("strings", `must be the table, an array, not ${describe(value)}`);
  const entries: readonly unknown[] = value;
  const strings: string[] = [];
  for (const [at, entry] of entries.entries()) {
    if (typeof entry !== "string") refuse("strings", `holds a ${typeof entry} at entry ${at}, not a string`);
    strings.push(entry);
  }
  return strings;
}

/** The node count the `u32` columns name: eight columns per node, or a whole number of nothing. */
function nodeCount(nodeCells: Uint32Array): number {
  if (nodeCells.length % NODE_COLUMNS !== 0) {
    refuse("nodeCells", `holds ${nodeCells.length} cells, not ${NODE_COLUMNS} per node`);
  }
  return nodeCells.length / NODE_COLUMNS;
}

function edgeCount(edgeCells: Uint32Array): number {
  if (edgeCells.length % EDGE_COLUMNS !== 0) {
    refuse("edgeCells", `holds ${edgeCells.length} cells, not ${EDGE_COLUMNS} per edge`);
  }
  return edgeCells.length / EDGE_COLUMNS;
}

/**
 * The three `f64` columns against the counts the contract states: `node weight` and `node
 * version` are one per node and `edge strength` one per edge (`ingest-columns.md:19-21`), and the
 * `u32` node columns are one per node (`:22-29`). A document whose counts disagree is refused by
 * the motor too, but only after the bytes are written, and one that disagrees here opens blank
 * instead — `Document.nodes` would be a different length from the motor's dense order.
 */
function counts(rows: Record<string, unknown>, nodes: number, edges: number): void {
  const weights = column(rows, "weights");
  const versions = column(rows, "versions");
  const strengths = column(rows, "strengths");
  if (weights.length !== nodes) refuse("weights", `holds ${weights.length} values for ${nodes} node rows`);
  if (versions.length !== nodes) refuse("versions", `holds ${versions.length} values for ${nodes} node rows`);
  if (strengths.length !== edges) refuse("strengths", `holds ${strengths.length} values for ${edges} edge rows`);
}

/**
 * The rows a host handed over, as the shape the worker's assembler takes, or a `TypeError` naming
 * the first field that is not what `ColumnRowsLike` says it is.
 */
export function columnRows(given: unknown): ColumnRowsLike {
  if (!isRecord(given)) refuse("rows", `must be the columns, an object, not ${describe(given)}`);
  const strings = table(given);
  const nodeCells = cells(given, "nodeCells");
  const edgeCells = cells(given, "edgeCells");
  counts(given, nodeCount(nodeCells), edgeCount(edgeCells));
  return {
    strings,
    nodeCells,
    edgeCells,
    weights: column(given, "weights"),
    versions: column(given, "versions"),
    strengths: column(given, "strengths"),
  };
}
/**
 * The embed example's columnar document: `fixtures/force/clustered.json` as the eight `u32`
 * columns per side that `docs/contract/ingest-columns.md` names, ready for the host verb
 * `loadColumns` (`packages/graph-studio/src/host/contract.ts:65`). One export, `columnsOf`.
 *
 * WHY the column order is the contract's and not this file's: the format is structure-of-arrays,
 * so cell `count + r` is row `r` of the second column and `count` is derived from the first
 * column's length — the reader counts its rows out of the two cell arrays alone
 * (`ingest-columns.md:39-40`), so a column order of our own would be read as the contract's.
 *
 * WHY an edge endpoint is a node **row** and not a string index: rows are dense and in the order
 * the document lists them, and `index_columns` refuses a repeated id instead of de-duplicating it,
 * so the row written here is the index the motor holds and an endpoint row names exactly that node
 * (`ingest-columns.md:62-75`). That is why a repeated node id is thrown below, not merged, and why
 * an endpoint id is resolved to its row before it is written.
 *
 * The values filled in are the ones `normaliseIngest` fills for this document: node `kind`
 * "record", `source` "file", `label` the node's own id, `weight` 0.5, `version` 0; edge `kind`
 * "relation", `label` the kind, `strength` the document's own value (`source/ingest.ts:190-254`).
 * `u32::MAX` is an absent optional column (`ingest-columns.md:51-53`).
 */
import { EDGE_COLUMNS, NODE_COLUMNS } from "../../packages/graph-studio/src/source/synthetic-columns.ts";
import type { ColumnRowsLike } from "../../packages/graph-studio/src/source/synthetic-columns.ts";

/** `u32::MAX` in an optional column: the field is absent, exactly as the JSON reader's `null`. */
const ABSENT = 0xffff_ffff;
/** The kinds the motor resolves (`source/ingest.ts:23-24`); anything else is refused. */
const NODE_KINDS: readonly string[] = ["record", "note", "database", "tag"];
const EDGE_KINDS: readonly string[] = ["relation", "tag", "note_of", "note_link", "hierarchy"];

/** Every string once, in first-seen order; the columns name entries, never the strings. */
class Table {
  readonly strings: string[] = [];
  readonly index = new Map<string, number>();

  of(text: string): number {
    const found = this.index.get(text);
    if (found !== undefined) return found;
    const next = this.strings.length;
    this.strings.push(text);
    this.index.set(text, next);
    return next;
  }
}

/** A guard, not a cast: the parsed document is outside every type in this tree. */
function isRecord(given: unknown): given is Record<string, unknown> {
  return typeof given === "object" && given !== null && !Array.isArray(given);
}

function record(at: string, given: unknown): Record<string, unknown> {
  if (!isRecord(given)) throw new Error(`${at} is not a JSON object`);
  return given;
}

function list(at: string, given: unknown): readonly unknown[] {
  if (!Array.isArray(given)) throw new Error(`${at} is not an array`);
  const items: readonly unknown[] = given;
  return items;
}

function stringOf(rec: Record<string, unknown>, key: string, at: string): string {
  const value = rec[key];
  if (typeof value !== "string") throw new Error(`${at}.${key} is not a string`);
  return value;
}

function optionalString(rec: Record<string, unknown>, key: string, at: string): string | null {
  const value = rec[key];
  if (value === undefined || value === null) return null;
  if (typeof value !== "string") throw new Error(`${at}.${key} is neither a string nor absent`);
  return value;
}

function optionalNumber(rec: Record<string, unknown>, key: string, at: string, fallback: number): number {
  const value = rec[key];
  if (value === undefined || value === null) return fallback;
  if (typeof value !== "number") throw new Error(`${at}.${key} is not a number`);
  return value;
}

function optionalBoolean(rec: Record<string, unknown>, key: string, at: string, fallback: boolean): boolean {
  const value = rec[key];
  if (value === undefined || value === null) return fallback;
  if (typeof value !== "boolean") throw new Error(`${at}.${key} is not a boolean`);
  return value;
}

function nodeKindOf(node: Record<string, unknown>, at: string): string {
  const kind = optionalString(node, "kind", at) ?? "record";
  if (!NODE_KINDS.includes(kind)) throw new Error(`${at}.kind ${JSON.stringify(kind)} is not a node kind (${NODE_KINDS.join(", ")})`);
  return kind;
}

function edgeKindOf(edge: Record<string, unknown>, at: string): string {
  const kind = optionalString(edge, "kind", at) ?? "relation";
  if (!EDGE_KINDS.includes(kind)) throw new Error(`${at}.kind ${JSON.stringify(kind)} is not an edge kind (${EDGE_KINDS.join(", ")})`);
  return kind;
}

/** An optional column's cell: `u32::MAX` when the document leaves the field out. */
function optionalCell(table: Table, text: string | null): number {
  return text === null ? ABSENT : table.of(text);
}

/** A `0`/`1` column (`has_note`, `directed`, `child_first`): `ingest-columns.md:29,36-37`. */
function flag(value: boolean): number {
  return value ? 1 : 0;
}

/** The columns being filled: the string table, both endpoint lookups, and the five arrays. */
class Builder {
  readonly table = new Table();
  readonly nodeRows = new Map<string, number>();
  readonly edgeIds = new Set<string>();
  readonly nodeCells: Uint32Array;
  readonly edgeCells: Uint32Array;
  readonly weights: Float64Array;
  readonly versions: Float64Array;
  readonly strengths: Float64Array;
  private readonly nodeCount: number;
  private readonly edgeCount: number;

  constructor(nodes: number, edges: number) {
    this.nodeCount = nodes;
    this.edgeCount = edges;
    this.nodeCells = new Uint32Array(nodes * NODE_COLUMNS);
    this.edgeCells = new Uint32Array(edges * EDGE_COLUMNS);
    this.weights = new Float64Array(nodes);
    this.versions = new Float64Array(nodes);
    this.strengths = new Float64Array(edges);
  }

  /** One row's eight cells, scattered: cell `count + r` of a column is row `r` of the *next*
   *  one (`ingest-columns.md:39-40`), so the eight columns are eight runs, not one row of eight.
   *  `count` is the final row count, which is why it is a field and not a running size. */
  private fill(cells: Uint32Array, count: number, row: number, values: readonly number[]): void {
    for (const [column, cell] of values.entries()) cells[column * count + row] = cell;
  }

  /** One node row, in the document's own order; eight columns, contract order. */
  node(given: unknown, row: number): void {
    const at = `nodes[${row}]`;
    const node = record(at, given);
    const id = stringOf(node, "id", at);
    const taken = this.nodeRows.get(id);
    if (taken !== undefined) throw new Error(`${at}: node id ${JSON.stringify(id)} is already row ${taken}, and a repeated id is an error, not a merge (ingest-columns.md:66-73)`);
    this.nodeRows.set(id, row);
    const kind = nodeKindOf(node, at);
    this.fill(this.nodeCells, this.nodeCount, row, [
      this.table.of(id),
      this.table.of(kind),
      optionalCell(this.table, optionalString(node, "database_id", at)),
      this.table.of(optionalString(node, "source", at) ?? "file"),
      this.table.of(optionalString(node, "label", at) ?? id),
      optionalCell(this.table, optionalString(node, "group", at)),
      optionalCell(this.table, optionalString(node, "icon", at)),
      flag(optionalBoolean(node, "has_note", at, kind === "note")),
    ]);
    this.weights[row] = optionalNumber(node, "weight", at, 0.5);
    this.versions[row] = optionalNumber(node, "version", at, 0);
  }

  /** The row of the node an endpoint id names, or a refusal naming the edge and the end. */
  private endpoint(edgeId: string, at: string, end: string, id: string): number {
    const found = this.nodeRows.get(id);
    if (found === undefined) throw new Error(`${at}: ${end} ${JSON.stringify(id)} of edge ${JSON.stringify(edgeId)} names no node`);
    return found;
  }

  /** One edge row: id, source row, target row, kind, label, record_id, directed, child_first. */
  edge(given: unknown, row: number): void {
    const at = `edges[${row}]`;
    const edge = record(at, given);
    const id = stringOf(edge, "id", at);
    if (this.edgeIds.has(id)) throw new Error(`${at}: edge id ${JSON.stringify(id)} is taken, and a repeated id is an error (ingest-columns.md:66-73)`);
    this.edgeIds.add(id);
    const kind = edgeKindOf(edge, at);
    const source = this.endpoint(id, at, "source", stringOf(edge, "source", at));
    const target = this.endpoint(id, at, "target", stringOf(edge, "target", at));
    this.fill(this.edgeCells, this.edgeCount, row, [
      this.table.of(id),
      source,
      target,
      this.table.of(kind),
      this.table.of(optionalString(edge, "label", at) ?? kind),
      optionalCell(this.table, optionalString(edge, "record_id", at)),
      flag(optionalBoolean(edge, "directed", at, kind === "hierarchy")),
      flag(optionalBoolean(edge, "child_first", at, false)),
    ]);
    this.strengths[row] = optionalNumber(edge, "strength", at, 0.5);
  }

  rows(): ColumnRowsLike {
    return {
      strings: this.table.strings,
      nodeCells: this.nodeCells,
      edgeCells: this.edgeCells,
      weights: this.weights,
      versions: this.versions,
      strengths: this.strengths,
    };
  }
}

/** The parsed ingest document as the columnar document `loadColumns` takes. */
export function columnsOf(doc: unknown): ColumnRowsLike {
  const root = record("the document", doc);
  const nodes = list("nodes", root["nodes"]);
  const edges = list("edges", root["edges"]);
  const builder = new Builder(nodes.length, edges.length);
  for (const [row, given] of nodes.entries()) builder.node(given, row);
  for (const [row, given] of edges.entries()) builder.edge(given, row);
  return builder.rows();
}
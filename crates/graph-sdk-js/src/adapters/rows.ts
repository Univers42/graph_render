// The rows adapter: a flat, SQL-ish source shape → the ingest contract.
//
// **A pure mapping, and that is the whole claim.** This file decides what a source's
// fields *mean* (by reading the roles its schema declares) and writes them down. It
// does not decide what a graph looks like: no node ids are built here, no edges are
// derived, no strength is chosen, no tag hub is synthesised. All of that is
// `graph-core`'s `ingest::build`, once, for every adapter — which is the property that
// makes "any data source" true rather than aspirational, and the reason an adapter that
// grew graph logic would be the abstraction leaking.
//
// The source shape is deliberately the *boring* one: named columns, rows of values, no
// nesting beyond the table a row belongs to. Anything more elaborate is somebody else's
// problem; this is the general case the other adapter is measured against
// (`fixtures/ingest/rows.json` and `notion.json` are the same dataset in two shapes,
// and both must produce byte-identical contract documents).

// The cells, the finiteness rule and the byte comparison live in `cells.ts`, which
// `notion.ts` reads too: one predicate, one cell map, so the two adapters cannot drift on
// what a cell may hold or in what order it is written. `RowsAdapterError` moved there
// with them and is re-exported here, so the package's export list and every existing
// import path — `./adapters/rows.ts` first of all — are unchanged.
import { bad, cellValues, compareBytes, u32 } from "./cells.ts";
export { RowsAdapterError } from "./cells.ts";

/** The eight declared roles (`crates/graph-contract/src/ingest.rs`). Exactly these, and
 * nothing else: a schema naming a role outside the set is refused by the contract's
 * reader, which is where that check belongs — not here, and not by quietly mapping an
 * unknown role onto `scalar`. */
export const Role = {
  Title: "title",
  Label: "label",
  Group: "group",
  Tags: "tags",
  Link: "link",
  Scalar: "scalar",
  Weight: "weight",
  Parent: "parent",
} as const;

/** A declared role's wire name. */
export type RoleName = (typeof Role)[keyof typeof Role];

/** The two link cardinalities. */
export const Cardinality = {
  One: "one",
  Many: "many",
} as const;

/** A link cardinality's wire name. */
export type CardinalityName = (typeof Cardinality)[keyof typeof Cardinality];

/** What a `link` column points at. */
export interface RowsLink {
  /** Id of the table the references name. */
  readonly collection: string;
  /** How many references the column may hold: `"one"` or `"many"`. */
  readonly cardinality: CardinalityName;
  /** `false` for a directed relation (the default when the column does not say). */
  readonly symmetric?: boolean;
}

/** One declared column of a table. `role` is the declaration; there is no inference. */
export interface RowsColumn {
  /** The column's id, and the key a row's values use. */
  readonly name: string;
  /** Human label, for the edge a `link` column draws. Never matched against. */
  readonly label?: string;
  /** What the column means. */
  readonly role: RoleName;
  /** The link's target, for a `link` column and only then. */
  readonly link?: RowsLink;
}

/** One table: its declared columns, and its rows. */
export interface RowsTable {
  /** Stable id, the middle coordinate of every node id derived from this table. */
  readonly id: string;
  /** Human name. Carried for diagnostics; nothing derives from it. */
  readonly name?: string;
  /** Which column's value is a row's label. Must name a `title` column. */
  readonly titleColumn: string;
  /** Its declared columns, in declaration order. */
  readonly columns: readonly RowsColumn[];
  /** Its rows. A row is a flat map of column name to value; a missing key is an
   * absent cell, which is not the same fact as an empty one. */
  readonly rows: readonly RowsRecord[];
}

/** One row: its id, whether it is deleted, its version, and its cells. */
export interface RowsRecord {
  /** Stable id within its table. */
  readonly id: string;
  /** Whether the source has deleted it. A deleted row derives nothing at all. */
  readonly deleted?: boolean;
  /** Source-assigned version, a `u32`. Defaults to `0` when the source has none. */
  readonly updatedAt?: number;
  /** Its cells, keyed by column name. */
  readonly values: Readonly<Record<string, unknown>>;
}

/** A whole rows source. */
export interface RowsSource {
  /** The backend these records came from; the first coordinate of every node id. */
  readonly source: string;
  /** Its tables, in the order the source lists them. */
  readonly tables: readonly RowsTable[];
}

// ------------------------------------------------------------------ the contract

/** A cell value: the whole of JSON, nothing more, so a source's own types survive. */
export type JsonValue = null | boolean | number | string | JsonValue[] | { [key: string]: JsonValue };

/** What a `link` field points at. */
export interface IngestLink {
  readonly collection: string;
  readonly cardinality: CardinalityName;
  readonly symmetric: boolean;
}

/** One declared field of a collection. */
export interface IngestField {
  readonly id: string;
  readonly name: string;
  readonly role: RoleName;
  /** Present on every field, `null` for every role but `link`: "no link" is stated,
   * never omitted, and the contract's reader requires the member. */
  readonly link: IngestLink | null;
}

/** A collection of records and the roles of its fields. */
export interface IngestCollection {
  readonly id: string;
  readonly name: string;
  readonly titleField: string;
  readonly fields: readonly IngestField[];
}

/** One record: its identity, its collection, and its values keyed by field id. */
export interface IngestRecord {
  readonly id: string;
  readonly collection: string;
  readonly deleted: boolean;
  readonly updatedAt: number;
  readonly values: Readonly<Record<string, JsonValue>>;
}

/** A whole ingest document: what every adapter writes. */
export interface Ingest {
  readonly version: 1;
  readonly source: string;
  readonly collections: readonly IngestCollection[];
  readonly records: readonly IngestRecord[];
}

// --------------------------------------------------------------- the mapping

function field(column: RowsColumn, path: string): IngestField {
  const link = column.role === Role.Link ? column.link : undefined;
  if (column.role === Role.Link && link === undefined) {
    bad(path, "a `link` column must declare its `link`");
  }
  if (column.role !== Role.Link && link !== undefined) {
    bad(path, "only a `link` column may declare a `link`");
  }
  return {
    id: column.name,
    // The column's label, or its name: the contract says a field's `name` is for
    // diagnostics, and this is the one place a human string is what a consumer reads.
    name: column.label ?? column.name,
    role: column.role,
    link: link === undefined ? null : wireLink(link, path),
  };
}

/** A declared link, written out. `symmetric` is read as itself or refused: the old
 * `link.symmetric === true` turned `"yes"` and `1` into `false` and drew the edge
 * directed, with nothing saying so. `cardinality` and `collection` are carried verbatim
 * because the contract's reader checks them, and refusing a name here would reject a
 * value the schema allows. An absent `symmetric` is the documented `false`. */
function wireLink(link: RowsLink, path: string): IngestLink {
  if (link.symmetric !== undefined && typeof link.symmetric !== "boolean") {
    bad(path, "a declared `link.symmetric` must be a boolean");
  }
  return { collection: link.collection, cardinality: link.cardinality, symmetric: link.symmetric ?? false };
}

function table(source: RowsTable, path: string): IngestCollection {
  // Sorted by column name, the contract's canonical order for a collection's fields —
  // so this adapter produces the same document whatever order the SQL happened to list
  // its columns in, and the same document `notion.ts` produces for the same dataset.
  const columns: { column: RowsColumn; path: string }[] = source.columns
    .map((column: RowsColumn, i: number) => ({ column, path: `${path}.columns[${i}]` }))
    .sort((a, b) => compareBytes(a.column.name, b.column.name));
  return {
    id: source.id,
    name: source.name ?? source.id,
    titleField: source.titleColumn,
    fields: columns.map(({ column, path }) => field(column, path)),
  };
}

function record(table: RowsTable, row: RowsRecord, path: string): IngestRecord {
  const declared = table.columns.map((column: RowsColumn) => column.name);
  const cells: [string, JsonValue][] = [];
  for (const [column, value] of Object.entries(row.values)) {
    if (!declared.includes(column)) {
      bad(`${path}.values`, `table \`${table.id}\` declares no column \`${column}\``);
    }
    cells.push([column, value as JsonValue]);
  }
  return {
    id: row.id,
    collection: table.id,
    deleted: row.deleted === true,
    // An absent version is the documented `0`; one that is present is the contract's
    // `u32`, so `-1`, `1.5`, `2**32` and `NaN` are refused here with the row to hand
    // rather than reaching a reader that refuses them for the wrong reason.
    updatedAt: row.updatedAt === undefined ? 0 : u32(row.updatedAt, `${path}.updatedAt`),
    values: cellValues(cells, `${path}.values`),
  };
}

/** Maps a rows source to the ingest contract, or refuses with the path that says where.
 *
 * Pure: the same source always gives the same document, and the document says nothing
 * this file decided beyond each column's declared role.
 */
export function rowsToIngest(source: RowsSource): Ingest {
  const collections = source.tables.map((t, i) => table(t, `tables[${i}]`));
  const records: IngestRecord[] = [];
  source.tables.forEach((t, i) => {
    t.rows.forEach((row, j) => records.push(record(t, row, `tables[${i}].rows[${j}]`)));
  });
  return { version: 1, source: source.source, collections, records };
}

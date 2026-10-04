/**
 * What the session hands the motor, and how each kind of source becomes it.
 *
 * Split out of `session.ts` so that file stays inside the house's line limit: these are the
 * *document* side of a load and touch none of the session's state.
 *
 * A generated graph leaves here as the binary columnar document
 * (`docs/contract/ingest-columns.md`) — no JSON text and no `IngestEdge` object is made on that
 * path any more, and neither is one made for the host's own rows. A document or a bundled fixture
 * leaves here as normalised JSON, unchanged: normalisation is the JSON reader's contract and the
 * columns path would be a second way to spell it.
 */
import { FIXTURES } from "../source/fixtures.ts";
import { type IngestNode, IngestRefusal, NODE_KINDS, normaliseIngest } from "../source/ingest.ts";
import type { NodeKind } from "../source/ingest.ts";
import { EDGE_COLUMNS, syntheticColumns } from "../source/synthetic-columns.ts";
import type { ColumnRowsLike } from "../source/synthetic-columns.ts";
import type { Source } from "../state/settings.ts";

/**
 * The wire form. `columns` is the generator's own output assembled by the SDK; `json` is the
 * text `gm_build` reads. Which one a document carries is decided here and read once, in
 * `replace` — nothing downstream of the build cares.
 */
export type Payload =
  | { readonly kind: "json"; readonly text: string }
  | { readonly kind: "columns"; readonly bytes: Uint8Array };

export interface Document {
  readonly name: string;
  readonly payload: Payload;
  readonly nodes: readonly IngestNode[];
  readonly edgeCount: number;
  readonly notes: readonly string[];
}

/** Assembles a document's columns. Injected rather than imported: `session.ts` and this file
 *  may not import the motor's SDK (`app/eslint.config.js`), and the SDK is what knows how to
 *  turn columns into bytes. */
export type Assembler = (rows: ColumnRowsLike) => Uint8Array;

/** The generated graph, as columns. The nodes are the same objects `syntheticRecords` makes,
 *  because the studio UI reads weights and kinds off them. */
function generated(source: Extract<Source, { kind: "synthetic" }>, assemble: Assembler): Document {
  const { rows, nodes, edgeCount } = syntheticColumns({
    seed: source.seed, nodeCount: source.nodes, degree: source.degree, shape: source.shape,
  });
  return {
    name: `${source.shape} seed ${source.seed}`,
    payload: { kind: "columns", bytes: assemble(rows) },
    nodes, edgeCount, notes: [],
  };
}

/** A document or a fixture: normalised, and still JSON. `normaliseIngest` records every
 *  default it filled and every annotation it dropped, and the UI shows them. */
function normalised(text: string, name: string): Document {
  const { json, doc, notes } = normaliseIngest(text, name);
  return {
    name, payload: { kind: "json", text: json },
    nodes: doc.nodes, edgeCount: doc.edges.length, notes,
  };
}

/** `u32::MAX` in an optional column: the field is absent, as the JSON reader's `null` is
 *  (`docs/contract/ingest-columns.md:51-53`). */
const ABSENT = 0xffff_ffff;

/** The table's own string, or null where `u32::MAX` says the field is absent. */
function optional(rows: ColumnRowsLike, cell: number): string | null {
  return cell === ABSENT ? null : rows.strings[cell] ?? null;
}

/**
 * The kind the table names, as the union member the UI reads. An unknown name falls back to
 * `record`, which is unreachable for a graph that opened: `gm_build_columns` refuses a kind that
 * resolves to nothing first (`ingest-columns.md:57-60`), so the build has already failed and no
 * node of this document is ever drawn.
 */
function kindOf(name: string | null): NodeKind {
  return NODE_KINDS.find((kind) => kind === name) ?? "record";
}

/**
 * One `IngestNode` per row, read out of the table and the eight node columns — the same shape the
 * generator hands over (`synthetic-columns.ts`, `table` and `nodeColumns`), so `Document.nodes`
 * is load-bearing here as it is there and `metaOf` never sees an empty column.
 *
 * Caveat: nothing measures this at 1M rows. It is 1M objects of twelve fields, and the only
 * related number is the generator's own 1389 → 1621 ms at 1M, which builds those objects *and*
 * fills six typed arrays and is called "+232, inside the spread"
 * (`docs/measurements/perf-open-synth-columns.md:74`). A host that already holds the objects pays
 * this pass again here; the way out is a `Document` that carries ids and columns only.
 */
function columnNodes(rows: ColumnRowsLike): readonly IngestNode[] {
  const cells = rows.nodeCells;
  const count = rows.weights.length;
  const nodes: IngestNode[] = [];
  for (let r = 0; r < count; r += 1) {
    const kind = kindOf(optional(rows, cells[count + r] ?? ABSENT));
    nodes.push({
      id: rows.strings[cells[r] ?? ABSENT] ?? "",
      kind,
      database_id: optional(rows, cells[2 * count + r] ?? ABSENT),
      source: rows.strings[cells[3 * count + r] ?? ABSENT] ?? "",
      label: rows.strings[cells[4 * count + r] ?? ABSENT] ?? "",
      group: optional(rows, cells[5 * count + r] ?? ABSENT),
      weight: rows.weights[r] ?? 0,
      version: rows.versions[r] ?? 0,
      has_note: cells[7 * count + r] === 1,
      icon: optional(rows, cells[6 * count + r] ?? ABSENT),
      tags: [],
      path: "",
    });
  }
  return nodes;
}

/**
 * A host's own columnar document: assembled here, in the worker, by the `Assembler` — the main
 * thread never imports the motor's SDK (`app/eslint.config.js`). The payload is bytes, so the
 * normaliser is not on this path at all: these rows are already its normal form, and a second
 * reader of them would be the "second way to spell it" this module's own header refuses.
 */
function columnar(source: Extract<Source, { kind: "columns" }>, assemble: Assembler): Document {
  const { rows } = source;
  return {
    name: source.name,
    payload: { kind: "columns", bytes: assemble(rows) },
    nodes: columnNodes(rows), edgeCount: rows.edgeCells.length / EDGE_COLUMNS, notes: [],
  };
}

export async function documentFor(
  source: Source,
  fixturesUrl: string,
  fetchText: (url: string) => Promise<string>,
  assemble: Assembler,
): Promise<Document> {
  if (source.kind === "synthetic") return generated(source, assemble);
  if (source.kind === "document") return normalised(source.text, source.name);
  if (source.kind === "columns") return columnar(source, assemble);
  // The path comes from settings, and settings come from recipes: only the listed files.
  if (!FIXTURES.includes(source.path)) throw new IngestRefusal(source.path, "not a bundled fixture");
  return normalised(await fetchText(`${fixturesUrl}${source.path}`), source.path);
}

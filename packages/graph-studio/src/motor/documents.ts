/**
 * What the session hands the motor, and how each kind of source becomes it.
 *
 * Split out of `session.ts` so that file stays inside the house's line limit: these three
 * functions are the *document* side of a load and touch none of the session's state.
 *
 * A generated graph leaves here as the binary columnar document
 * (`docs/contract/ingest-columns.md`) — no JSON text and no `IngestEdge` object is made on that
 * path any more. A document or a bundled fixture leaves here as normalised JSON, unchanged:
 * normalisation is the JSON reader's contract and the columns path would be a second way to
 * spell it.
 */
import { FIXTURES } from "../source/fixtures.ts";
import { type IngestNode, IngestRefusal, normaliseIngest } from "../source/ingest.ts";
import { syntheticColumns } from "../source/synthetic-columns.ts";
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

export async function documentFor(
  source: Source,
  fixturesUrl: string,
  fetchText: (url: string) => Promise<string>,
  assemble: Assembler,
): Promise<Document> {
  if (source.kind === "synthetic") return generated(source, assemble);
  if (source.kind === "document") return normalised(source.text, source.name);
  // The path comes from settings, and settings come from recipes: only the listed files.
  if (!FIXTURES.includes(source.path)) throw new IngestRefusal(source.path, "not a bundled fixture");
  return normalised(await fetchText(`${fixturesUrl}${source.path}`), source.path);
}

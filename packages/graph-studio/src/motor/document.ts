/**
 * What one source is before the motor has it: the document to build from, the nodes the studio
 * describes with, and what reading it filled in or had to drop.
 *
 * Its own module so the session is about running a graph and this is about reading one; the
 * two change for different reasons, and `session.ts` was over the house's 300 lines with both.
 */
import { type IngestNode, IngestRefusal, normaliseIngest } from "../source/ingest.ts";
import { FIXTURES } from "../source/fixtures.ts";
import { syntheticRecords } from "../source/synthetic.ts";
import type { Source } from "../state/settings.ts";

export interface Document {
  readonly name: string;
  readonly json: string;
  readonly nodes: readonly IngestNode[];
  readonly edgeCount: number;
  readonly notes: readonly string[];
}

function generated(source: Extract<Source, { kind: "synthetic" }>): Document {
  const { nodes, edges } = syntheticRecords({
    seed: source.seed, nodeCount: source.nodes, degree: source.degree, shape: source.shape,
  });
  return {
    name: `${source.shape} seed ${source.seed}`,
    json: JSON.stringify({ version: 1, nodes, edges }),
    nodes, edgeCount: edges.length, notes: [],
  };
}

function normalised(text: string, name: string): Document {
  const { json, doc, notes } = normaliseIngest(text, name);
  return { name, json, nodes: doc.nodes, edgeCount: doc.edges.length, notes };
}

/**
 * The document `source` is, fetched where it has to be. A document source is the text the
 * caller holds; a fixture is one of the listed files, and the path comes from settings, which
 * come from recipes — so an unlisted path is refused here rather than fetched.
 */
export async function documentFor(
  source: Source,
  fixturesUrl: string,
  fetchText: (url: string) => Promise<string>,
): Promise<Document> {
  if (source.kind === "synthetic") return generated(source);
  if (source.kind === "document") return normalised(source.text, source.name);
  if (!FIXTURES.includes(source.path)) throw new IngestRefusal(source.path, "not a bundled fixture");
  return normalised(await fetchText(`${fixturesUrl}${source.path}`), source.path);
}

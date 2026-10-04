/** A source turned into the ingest document the motor builds, with what the summary reports of it. */
import type { Source } from "../state/settings.ts";
import { FIXTURES } from "./fixtures.ts";
import { type IngestNode, IngestRefusal, normaliseIngest } from "./ingest.ts";
import { syntheticRecords } from "./synthetic.ts";

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

export async function documentFor(source: Source, fixturesUrl: string, fetchText: (url: string) => Promise<string>): Promise<Document> {
  if (source.kind === "synthetic") return generated(source);
  if (source.kind === "document") return normalised(source.text, source.name);
  // The path comes from settings, and settings come from recipes: only the listed files.
  if (!FIXTURES.includes(source.path)) throw new IngestRefusal(source.path, "not a bundled fixture");
  return normalised(await fetchText(`${fixturesUrl}${source.path}`), source.path);
}

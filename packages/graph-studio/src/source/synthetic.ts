/**
 * A generated graph, as a provisional ingest document. Nothing here computes a position:
 * the document goes to `gm_build` like any other.
 *
 * Two shapes. `random` is the first studio's model, kept byte for byte because the perf
 * baseline was measured on it: node `i` links to uniformly drawn earlier nodes. `vault`
 * looks like a notes vault: a few topics, a hub per topic, links that prefer the same
 * topic and the nodes that already have links.
 *
 * The draws live in `synthetic-draw.ts`, which the studio's column path
 * (`synthetic-columns.ts`) reads too: one set of draws, two wire formats, and neither a copy
 * of the other. This module is the JSON half, and it is what `syntheticIngest` and the perf
 * harnesses call; the column half never builds an `IngestEdge` and never calls
 * `JSON.stringify`.
 *
 * Deterministic: mulberry32 with integer steps, no clock, no `Math.random`, and a fixed draw
 * order per shape.
 */
import type { IngestDoc, IngestEdge, IngestNode } from "./ingest.ts";
import {
  EDGE_KIND_NAMES,
  EDGE_STRENGTH,
  applyDegreeWeights,
  drawGraph,
} from "./synthetic-draw.ts";
import type { Drawn, SyntheticSpec } from "./synthetic-draw.ts";

export { MAX_DEGREE, MAX_NODES, SHAPES, titleOf } from "./synthetic-draw.ts";
export type { SyntheticShape, SyntheticSpec } from "./synthetic-draw.ts";

interface Records {
  readonly nodes: IngestNode[];
  readonly edges: IngestEdge[];
}

/** The edges, as records, from the columns. An endpoint that names no node cannot happen: both
 *  generators draw a row for every endpoint, and the `??` names the id that row would have had.
 *  This is the one place the JSON half resolves a row to an id — the columns half never does. */
function edgesOf(drawn: Drawn): IngestEdge[] {
  const edges: IngestEdge[] = [];
  for (let j = 0; j < drawn.edgeCount; j += 1) {
    const kind = EDGE_KIND_NAMES[drawn.edgeKinds[j] ?? 0] ?? "relation";
    const from = drawn.from[j] ?? 0;
    const to = drawn.to[j] ?? 0;
    edges.push({
      id: `e-${j}`,
      source: drawn.nodes[from]?.id ?? `n-${from}`,
      target: drawn.nodes[to]?.id ?? `n-${to}`,
      kind, label: kind, strength: EDGE_STRENGTH, directed: kind === "hierarchy",
      record_id: null, child_first: false,
    });
  }
  return edges;
}

/** The records the JSON document is built from: the draws, with the degree weights applied and
 *  the edges named by id. Same signature, same field values, same order as before the columns
 *  path existed — the ids come from the node that owns them, so an endpoint is a lookup and
 *  not a second concatenation. */
export function syntheticRecords(spec: SyntheticSpec): Records {
  const drawn = drawGraph(spec);
  applyDegreeWeights(drawn.nodes, drawn.counts);
  return { nodes: drawn.nodes, edges: edgesOf(drawn) };
}

/** Member order is the contract's field order, so one spec gives one byte string. */
export function syntheticIngest(spec: SyntheticSpec): string {
  const { nodes, edges } = syntheticRecords(spec);
  const doc: IngestDoc = { version: 1, nodes, edges };
  return JSON.stringify(doc);
}

/**
 * The synthetic graph, as a provisional ingest document.
 *
 * The wasm module's own seed export (`gm_seed_ingest`) is deliberately NOT on
 * the published SDK surface — it exists for the hash gate at a fixed node count
 * and reference degree, neither of which the studio can vary. So the studio
 * generates the same SHAPE of document itself, from the same mulberry32 stream
 * `src/core/model/synthetic.ts` documents, and hands it to `gm_build` like any
 * other ingest. Everything downstream is the real engine: nothing here computes
 * a position.
 *
 * Deterministic by construction (D1/D3/D8): integer-only PRNG steps, no
 * transcendental, no clock, no `Math.random`, and the draw order is fixed —
 * two draws per node (kind, group), then two per edge (target, kind).
 */

import type { IngestDoc, IngestEdge, IngestNode } from "./ingestText.ts";

export interface SyntheticSpec {
  /** Any u32; the PRNG state is `seed >>> 0`. */
  readonly seed: number;
  /** Clamped to 2..2000 — enough to look like a graph, small enough to stay smooth. */
  readonly nodeCount: number;
  /** Reference degree, clamped to 0..12. 0 builds a node set with no edges. */
  readonly degree: number;
}

const GROUPS = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"] as const;
const DATABASES = 8;
export const MAX_NODES = 2000;
export const MAX_DEGREE = 12;

/** mulberry32 (`src/core/model/synthetic.ts:27`): integer steps, then `/ 2^32`,
 *  so every target sees the same stream. `Math.imul` is the 32-bit multiply. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(a ^ (a >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function clamp(value: number, low: number, high: number): number {
  const whole = Number.isFinite(value) ? Math.trunc(value) : low;
  return whole < low ? low : whole > high ? high : whole;
}

/** The kind roll: rare kinds, so a snapshot shows the shape language. */
function kindOf(roll: number): IngestNode["kind"] {
  if (roll < 0.06) return "note";
  if (roll < 0.12) return "tag";
  if (roll < 0.18) return "database";
  return "record";
}

/** The edge roll. `hierarchy` is the only directed kind the contract names. */
function edgeKindOf(roll: number): IngestEdge["kind"] {
  if (roll < 0.1) return "hierarchy";
  if (roll < 0.2) return "note_link";
  if (roll < 0.25) return "tag";
  return "relation";
}

function makeNodes(count: number, rnd: () => number): IngestNode[] {
  const nodes: IngestNode[] = [];
  for (let i = 0; i < count; i += 1) {
    const kind = kindOf(rnd());
    const group = GROUPS[Math.floor(rnd() * GROUPS.length)];
    nodes.push({
      id: `n-${i}`, kind, database_id: `db-${i % DATABASES}`, source: "studio",
      label: `Node ${i}`, group, weight: 0, version: 0, has_note: kind === "note", icon: null,
    });
  }
  return nodes;
}

/** Preferential attachment: node `i` reaches back `degree` times into the `i`
 *  nodes before it. Parallel edges are legal (distinct ids) and happen. */
function makeEdges(count: number, degree: number, rnd: () => number): IngestEdge[] {
  const edges: IngestEdge[] = [];
  for (let i = Math.max(1, degree); i < count; i += 1) {
    for (let k = 0; k < degree; k += 1) {
      // Draw order is load-bearing and fixed: the target index first, then the
      // kind roll. A reader that reorders these two produces a different graph
      // from the same seed, which is exactly what the pinned document forbids.
      const target = `n-${Math.floor(rnd() * i)}`;
      const kind = edgeKindOf(rnd());
      edges.push({
        id: `e-${edges.length}`, source: `n-${i}`, target,
        kind, label: kind, strength: 0.5, directed: kind === "hierarchy",
        record_id: null, child_first: false,
      });
    }
  }
  return edges;
}

/** Weight is the node's degree over the graph's maximum degree — the same
 *  degree-derived weight the engine's own synthetic model carries, so the studio's
 *  nodes size by connectivity the way a real model's would. */
function applyDegreeWeights(nodes: IngestNode[], edges: IngestEdge[]): void {
  const counts = new Map<string, number>(nodes.map((node) => [node.id, 0]));
  for (const edge of edges) {
    counts.set(edge.source, (counts.get(edge.source) ?? 0) + 1);
    counts.set(edge.target, (counts.get(edge.target) ?? 0) + 1);
  }
  const max = Math.max(0, ...counts.values());
  for (const node of nodes) node.weight = max === 0 ? 0 : (counts.get(node.id) ?? 0) / max;
}

/** The records behind the document, for the panels that show what was built. */
export function syntheticRecords(spec: SyntheticSpec): { nodes: IngestNode[]; edges: IngestEdge[] } {
  const count = clamp(spec.nodeCount, 2, MAX_NODES);
  const degree = clamp(spec.degree, 0, MAX_DEGREE);
  const rnd = mulberry32(spec.seed);
  const nodes = makeNodes(count, rnd);
  const edges = makeEdges(count, degree, rnd);
  applyDegreeWeights(nodes, edges);
  return { nodes, edges };
}

/** The ingest text `gm_build` reads. Member order is the contract's field order,
 *  so two documents for the same spec are byte-identical. */
export function syntheticIngest(spec: SyntheticSpec): string {
  const { nodes, edges } = syntheticRecords(spec);
  const doc: IngestDoc = { version: 1, nodes, edges };
  return JSON.stringify(doc);
}

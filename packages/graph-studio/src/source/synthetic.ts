/**
 * A generated graph, as a provisional ingest document. Nothing here computes a position:
 * the document goes to `gm_build` like any other.
 *
 * Two shapes. `random` is the first studio's model, kept byte for byte because the perf
 * baseline was measured on it: node `i` links to uniformly drawn earlier nodes. `vault`
 * looks like a notes vault: a few topics, a hub per topic, links that prefer the same
 * topic and the nodes that already have links.
 *
 * Deterministic: mulberry32 with integer steps, no clock, no `Math.random`, and a fixed
 * draw order per shape.
 */
import type { IngestDoc, IngestEdge, IngestNode } from "./ingest.ts";

export type SyntheticShape = "random" | "vault";

export interface SyntheticSpec {
  /** Any u32; the PRNG state is `seed >>> 0`. */
  readonly seed: number;
  /** Clamped to 2..MAX_NODES. */
  readonly nodeCount: number;
  /** Links each node makes to earlier nodes, clamped to 0..MAX_DEGREE. */
  readonly degree: number;
  /** Default `random`. */
  readonly shape?: SyntheticShape;
}

export const MAX_NODES = 50000;
export const MAX_DEGREE = 12;
export const SHAPES: readonly SyntheticShape[] = ["vault", "random"];

const GROUPS = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"] as const;
const TOPICS = [
  "Layout", "Memory", "Agents", "Rendering", "Storage", "Search",
  "Contracts", "Testing", "Hosts", "Analysis", "Caching", "Reading",
] as const;
const SUBJECTS = [
  "Graph", "Layout", "Memory", "Agent", "Vector", "Index", "Cache", "Schema",
  "Query", "Token", "Session", "Render", "Cluster", "Matrix", "Kernel", "Ledger",
] as const;
const FACETS = ["notes", "design", "review", "spec", "draft", "log", "ideas", "plan", "survey", "proof", "bench"] as const;
const DATABASES = 8;
const SAME_TOPIC = 0.85;

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

function pick<T>(items: readonly T[], roll: number, fallback: T): T {
  return items[Math.floor(roll * items.length)] ?? fallback;
}

/** A title per index, unique and drawn from no stream, so it costs the shapes no draw. */
export function titleOf(index: number): string {
  const subject = SUBJECTS[index % SUBJECTS.length] ?? "Graph";
  const facet = FACETS[Math.floor(index / SUBJECTS.length) % FACETS.length] ?? "notes";
  const round = Math.floor(index / (SUBJECTS.length * FACETS.length));
  return round === 0 ? `${subject} ${facet}` : `${subject} ${facet} ${round + 1}`;
}

function kindOf(roll: number): IngestNode["kind"] {
  if (roll < 0.06) return "note";
  if (roll < 0.12) return "tag";
  if (roll < 0.18) return "database";
  return "record";
}

function edgeKindOf(roll: number): IngestEdge["kind"] {
  if (roll < 0.1) return "hierarchy";
  if (roll < 0.2) return "note_link";
  if (roll < 0.25) return "tag";
  return "relation";
}

function nodeOf(index: number, kind: IngestNode["kind"], group: string): IngestNode {
  return {
    id: `n-${index}`, kind, database_id: `db-${index % DATABASES}`, source: "studio",
    label: titleOf(index), group, weight: 0, version: 0, has_note: kind === "note", icon: null,
  };
}

function edgeOf(index: number, from: number, to: number, kind: IngestEdge["kind"]): IngestEdge {
  return {
    id: `e-${index}`, source: `n-${from}`, target: `n-${to}`,
    kind, label: kind, strength: 0.5, directed: kind === "hierarchy",
    record_id: null, child_first: false,
  };
}

interface Records {
  readonly nodes: IngestNode[];
  readonly edges: IngestEdge[];
}

/** Draw order: per node (kind, group), then per edge (target, kind). */
function randomRecords(count: number, degree: number, rnd: () => number): Records {
  const nodes: IngestNode[] = [];
  for (let i = 0; i < count; i += 1) {
    const kind = kindOf(rnd());
    nodes.push(nodeOf(i, kind, pick(GROUPS, rnd(), "Alpha")));
  }
  const edges: IngestEdge[] = [];
  for (let i = Math.max(1, degree); i < count; i += 1) {
    for (let k = 0; k < degree; k += 1) {
      const target = Math.floor(rnd() * i);
      edges.push(edgeOf(edges.length, i, target, edgeKindOf(rnd())));
    }
  }
  return { nodes, edges };
}

interface Topic {
  readonly name: string;
  /** One entry per link end held by a node of this topic, plus one per node: drawing an
   *  entry uniformly is drawing a node in proportion to its links. */
  readonly ends: number[];
}

function topicsFor(count: number): Topic[] {
  const wanted = clamp(Math.round(Math.sqrt(count) / 2), 3, TOPICS.length);
  return TOPICS.slice(0, wanted).map((name) => ({ name, ends: [] }));
}

function vaultTarget(topic: Topic, index: number, rnd: () => number): number {
  const stay = rnd() < SAME_TOPIC;
  const roll = rnd();
  if (stay && topic.ends.length > 0) return pick(topic.ends, roll, 0);
  return Math.floor(roll * index);
}

/** Draw order: per node (topic, kind), then per link (stay, target, kind). */
function vaultRecords(count: number, degree: number, rnd: () => number): Records {
  const topics = topicsFor(count);
  const fallback = topics[0] ?? { name: "Layout", ends: [] };
  const topicOf: Topic[] = [];
  const nodes: IngestNode[] = [];
  const edges: IngestEdge[] = [];
  for (let i = 0; i < count; i += 1) {
    const topic = pick(topics, rnd(), fallback);
    const kind = kindOf(rnd());
    nodes.push(nodeOf(i, topic.ends.length === 0 ? "database" : kind, topic.name));
    for (let k = 0; k < degree && i > 0; k += 1) {
      const target = vaultTarget(topic, i, rnd);
      edges.push(edgeOf(edges.length, i, target, edgeKindOf(rnd())));
      topicOf[target]?.ends.push(target);
      topic.ends.push(i);
    }
    topic.ends.push(i);
    topicOf.push(topic);
  }
  return { nodes, edges };
}

/** Weight is the node's link count over the graph's largest, so nodes size by connectivity. */
function applyDegreeWeights(records: Records): void {
  const counts = new Map<string, number>(records.nodes.map((node) => [node.id, 0]));
  let max = 0;
  for (const edge of records.edges) {
    for (const end of [edge.source, edge.target]) {
      const next = (counts.get(end) ?? 0) + 1;
      counts.set(end, next);
      if (next > max) max = next;
    }
  }
  for (const node of records.nodes) node.weight = max === 0 ? 0 : (counts.get(node.id) ?? 0) / max;
}

export function syntheticRecords(spec: SyntheticSpec): Records {
  const count = clamp(spec.nodeCount, 2, MAX_NODES);
  const degree = clamp(spec.degree, 0, MAX_DEGREE);
  const rnd = mulberry32(spec.seed);
  const records = spec.shape === "vault" ? vaultRecords(count, degree, rnd) : randomRecords(count, degree, rnd);
  applyDegreeWeights(records);
  return records;
}

/** Member order is the contract's field order, so one spec gives one byte string. */
export function syntheticIngest(spec: SyntheticSpec): string {
  const { nodes, edges } = syntheticRecords(spec);
  const doc: IngestDoc = { version: 1, nodes, edges };
  return JSON.stringify(doc);
}

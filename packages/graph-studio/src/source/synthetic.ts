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
import { type IngestDoc, type IngestEdge, type IngestNode, IngestRefusal } from "./ingest.ts";
import { linksRefusal } from "./limits.ts";

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

export const MAX_NODES = 1_000_000;
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

function nodeOf(index: number, kind: IngestNode["kind"], group: string, id: string): IngestNode {
  return {
    id, kind, database_id: `db-${index % DATABASES}`, source: "studio",
    label: titleOf(index), group, weight: 0, version: 0, has_note: kind === "note", icon: null,
  };
}

type EdgeWriter = (index: number, from: number, to: number, kind: IngestEdge["kind"]) => IngestEdge;

/** An edge names its endpoints by id, not by index — but the ids were built once each,
 *  with the node that owns them, so every edge end reuses that one string instead of
 *  concatenating a fresh one. The `??` is the same string the concatenation would have
 *  made: an endpoint outside `ids` is unreachable from either generator. */
function edgeWriter(ids: readonly string[]): EdgeWriter {
  return (index, from, to, kind) => ({
    id: `e-${index}`, source: ids[from] ?? `n-${from}`, target: ids[to] ?? `n-${to}`,
    kind, label: kind, strength: 0.5, directed: kind === "hierarchy",
    record_id: null, child_first: false,
  });
}

interface Records {
  readonly nodes: IngestNode[];
  readonly edges: IngestEdge[];
}

/** What a generator hands the weight pass: the records, and the link count per node index
 *  filled in as the edges were made — the generator knows both ends, so nothing is hashed. */
interface Built extends Records {
  readonly counts: Uint32Array;
}

/** One link end on one node. */
function bump(counts: Uint32Array, index: number): void {
  counts[index] = (counts[index] ?? 0) + 1;
}

/** Draw order: per node (kind, group), then per edge (target, kind). */
function randomRecords(count: number, degree: number, rnd: () => number): Built {
  const ids: string[] = new Array<string>(count);
  const counts = new Uint32Array(count);
  const nodes: IngestNode[] = [];
  for (let i = 0; i < count; i += 1) {
    const kind = kindOf(rnd());
    const id = `n-${i}`;
    ids[i] = id;
    nodes.push(nodeOf(i, kind, pick(GROUPS, rnd(), "Alpha"), id));
  }
  const edges: IngestEdge[] = [];
  const edge = edgeWriter(ids);
  for (let i = Math.max(1, degree); i < count; i += 1) {
    for (let k = 0; k < degree; k += 1) {
      const target = Math.floor(rnd() * i);
      edges.push(edge(edges.length, i, target, edgeKindOf(rnd())));
      bump(counts, i);
      bump(counts, target);
    }
  }
  return { nodes, edges, counts };
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
function vaultRecords(count: number, degree: number, rnd: () => number): Built {
  const topics = topicsFor(count);
  const fallback = topics[0] ?? { name: "Layout", ends: [] };
  const topicOf: Topic[] = [];
  const nodes: IngestNode[] = [];
  const edges: IngestEdge[] = [];
  const ids: string[] = new Array<string>(count);
  const counts = new Uint32Array(count);
  const edge = edgeWriter(ids);
  for (let i = 0; i < count; i += 1) {
    const topic = pick(topics, rnd(), fallback);
    const kind = kindOf(rnd());
    const id = `n-${i}`;
    ids[i] = id;
    nodes.push(nodeOf(i, topic.ends.length === 0 ? "database" : kind, topic.name, id));
    for (let k = 0; k < degree && i > 0; k += 1) {
      const target = vaultTarget(topic, i, rnd);
      edges.push(edge(edges.length, i, target, edgeKindOf(rnd())));
      bump(counts, i);
      bump(counts, target);
      topicOf[target]?.ends.push(target);
      topic.ends.push(i);
    }
    topic.ends.push(i);
    topicOf.push(topic);
  }
  return { nodes, edges, counts };
}

/** Weight is the node's link count over the graph's largest, so nodes size by connectivity.
 *  The counts arrive as node indices, filled in by the generator that made the edges: an id
 *  is never hashed, which at a million nodes was the whole cost of this pass. */
function applyDegreeWeights(built: Built): Records {
  let max = 0;
  for (const count of built.counts) if (count > max) max = count;
  const { nodes, edges, counts } = built;
  for (const [index, node] of nodes.entries()) {
    node.weight = max === 0 ? 0 : (counts[index] ?? 0) / max;
  }
  return { nodes, edges };
}

/** Refuses a graph past `MAX_LINKS` before it makes a record: the document would not fit. */
export function syntheticRecords(spec: SyntheticSpec): Records {
  const count = clamp(spec.nodeCount, 2, MAX_NODES);
  const degree = clamp(spec.degree, 0, MAX_DEGREE);
  const refused = linksRefusal(count, degree);
  if (refused !== null) throw new IngestRefusal("synthetic", refused);
  const rnd = mulberry32(spec.seed);
  const built = spec.shape === "vault" ? vaultRecords(count, degree, rnd) : randomRecords(count, degree, rnd);
  return applyDegreeWeights(built);
}

/** Member order is the contract's field order, so one spec gives one byte string. */
export function syntheticIngest(spec: SyntheticSpec): string {
  const { nodes, edges } = syntheticRecords(spec);
  const doc: IngestDoc = { version: 1, nodes, edges };
  return JSON.stringify(doc);
}

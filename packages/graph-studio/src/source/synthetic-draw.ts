/**
 * The generator's draws, kept apart from both consumers.
 *
 * `synthetic.ts` turns these into an `IngestEdge[]` for `JSON.stringify`; `synthetic-columns.ts`
 * turns the same draws into the binary document's columns. Both read *this*, so the draw order
 * — which decides every id, every endpoint and every weight — is written once
 * (`rules/library-first.md`: two functions that change for the same reason are one function in
 * the wrong place).
 *
 * Nothing here is an `IngestEdge`. An edge is two node **rows** and a kind index: the generator
 * draws a row for every endpoint it makes, so it never hashes an id to find one, and a consumer
 * that wants ids looks a row up in `nodes` instead.
 *
 * Deterministic: mulberry32 with integer steps, no clock, no `Math.random`, and a fixed draw
 * order per shape.
 */
import type { IngestNode, NodeKind } from "./ingest.ts";

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

/** The node kinds, in the order the columns' fixed head lists them. An index *is* the answer:
 *  the columns path needs a table index and the records path needs the name, so the draw
 *  produces the index once and both read it. */
export const NODE_KIND_NAMES = ["record", "note", "database", "tag"] as const satisfies readonly NodeKind[];
/** The edge kinds the generator draws, in the head's order. */
export const EDGE_KIND_NAMES = ["hierarchy", "note_link", "tag", "relation"] as const;
/** Every group a node can carry: `GROUPS` then `TOPICS`, so one `Uint8` holds either index. */
export const GROUP_NAMES: readonly string[] = [...GROUPS, ...TOPICS];
/** The one `source` every node carries. */
export const SOURCE_NAME = "studio";
/** `db-0` … `db-7`, which is every database the generator names. */
export const DATABASE_COUNT = DATABASES;

const NOTE_KIND = NODE_KIND_NAMES.indexOf("note");
const TAG_KIND = NODE_KIND_NAMES.indexOf("tag");
const DATABASE_KIND = NODE_KIND_NAMES.indexOf("database");
const RECORD_KIND = NODE_KIND_NAMES.indexOf("record");
const HIERARCHY_KIND = EDGE_KIND_NAMES.indexOf("hierarchy");
const NOTE_LINK_KIND = EDGE_KIND_NAMES.indexOf("note_link");
const TAG_EDGE_KIND = EDGE_KIND_NAMES.indexOf("tag");
const RELATION_KIND = EDGE_KIND_NAMES.indexOf("relation");
/** Every edge is at 0.5 and every node's `version` is 0: the columns path fills those whole
 *  columns rather than reading them off a record. */
export const EDGE_STRENGTH = 0.5;

export function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(a ^ (a >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function clamp(value: number, low: number, high: number): number {
  const whole = Number.isFinite(value) ? Math.trunc(value) : low;
  return whole < low ? low : whole > high ? high : whole;
}

/** An index into `items` from a roll in [0, 1). The bound check is the same fallback `pick`
 *  had: unreachable while `rnd()` stays below 1, which mulberry32 guarantees, and a named
 *  `0` rather than a silent `undefined` if it ever does not. */
function pickIndex(length: number, roll: number): number {
  const at = Math.floor(roll * length);
  return at >= 0 && at < length ? at : 0;
}

/** A title per index, unique and drawn from no stream, so it costs the shapes no draw. */
export function titleOf(index: number): string {
  const subject = SUBJECTS[index % SUBJECTS.length] ?? "Graph";
  const facet = FACETS[Math.floor(index / SUBJECTS.length) % FACETS.length] ?? "notes";
  const round = Math.floor(index / (SUBJECTS.length * FACETS.length));
  return round === 0 ? `${subject} ${facet}` : `${subject} ${facet} ${round + 1}`;
}

/** The node kind index, from one roll. The thresholds are the shape's, unchanged. */
function nodeKindIndex(roll: number): number {
  if (roll < 0.06) return NOTE_KIND;
  if (roll < 0.12) return TAG_KIND;
  if (roll < 0.18) return DATABASE_KIND;
  return RECORD_KIND;
}

/** The edge kind index, from one roll. */
function edgeKindIndex(roll: number): number {
  if (roll < 0.1) return HIERARCHY_KIND;
  if (roll < 0.2) return NOTE_LINK_KIND;
  if (roll < 0.25) return TAG_EDGE_KIND;
  return RELATION_KIND;
}

/** The node, with its kind and group already resolved to names. Its id is built once here and
 *  reused by every edge that ends on it, so no endpoint concatenates a second copy. */
function nodeOf(index: number, kind: number, group: number, id: string): IngestNode {
  return {
    id, kind: NODE_KIND_NAMES[kind] ?? "record",
    database_id: `db-${index % DATABASES}`, source: SOURCE_NAME,
    label: titleOf(index), group: GROUP_NAMES[group] ?? "Alpha",
    weight: 0, version: 0, has_note: kind === NOTE_KIND, icon: null,
  };
}

/** One link end on one node. */
function bump(counts: Uint32Array, index: number): void {
  counts[index] = (counts[index] ?? 0) + 1;
}

/** One graph, as columns. `from`/`to` are node **rows**, `kinds`/`groups`/`edgeKinds` are
 *  indices into the fixed name lists above, and `counts` is the link-end tally the weight pass
 *  reads — filled in as the edges were made, so nothing is hashed to find a node's row. */
export interface Drawn {
  readonly nodes: IngestNode[];
  readonly kinds: Uint8Array;
  readonly groups: Uint8Array;
  readonly from: Uint32Array;
  readonly to: Uint32Array;
  readonly edgeKinds: Uint8Array;
  readonly counts: Uint32Array;
  readonly edgeCount: number;
}

/** How many edges a shape draws, known before the first draw: both loops run `degree`
 *  iterations over a fixed set of nodes. `random` starts at `max(1, degree)`, because a node
 *  with no earlier node has no endpoint to name; `vault` skips node 0 alone. Exact, so the
 *  three edge columns are allocated once and never grown. */
function plannedEdges(count: number, degree: number, shape: SyntheticShape | undefined): number {
  if (shape === "vault") return degree * (count - 1);
  return degree * Math.max(0, count - Math.max(1, degree));
}

/** The filled count against the planned one. A mismatch is a bug in `plannedEdges`, not a bad
 *  spec, and letting it pass would write the next edge over the row after the last one. */
function checkEdges(shape: SyntheticShape | undefined, filledCount: number, planned: number): void {
  if (filledCount !== planned) {
    throw new Error(`${shape ?? "random"}: drew ${filledCount} edges, planned ${planned}`);
  }
}

/** Draw order: per node (kind, group), then per edge (target, kind). */
function randomDraw(count: number, degree: number, rnd: () => number, planned: number): Drawn {
  const nodes: IngestNode[] = [];
  const kinds = new Uint8Array(count);
  const groups = new Uint8Array(count);
  const counts = new Uint32Array(count);
  const from = new Uint32Array(planned);
  const to = new Uint32Array(planned);
  const edgeKinds = new Uint8Array(planned);
  let edgeCount = 0;
  for (let i = 0; i < count; i += 1) {
    const kind = nodeKindIndex(rnd());
    const group = pickIndex(GROUPS.length, rnd());
    kinds[i] = kind;
    groups[i] = group;
    nodes.push(nodeOf(i, kind, group, `n-${i}`));
  }
  for (let i = Math.max(1, degree); i < count; i += 1) {
    for (let k = 0; k < degree; k += 1) {
      const target = Math.floor(rnd() * i);
      from[edgeCount] = i;
      to[edgeCount] = target;
      edgeKinds[edgeCount] = edgeKindIndex(rnd());
      edgeCount += 1;
      bump(counts, i);
      bump(counts, target);
    }
  }
  checkEdges("random", edgeCount, planned);
  return { nodes, kinds, groups, from, to, edgeKinds, counts, edgeCount };
}

interface Topic {
  readonly name: string;
  /** This topic's index into `GROUP_NAMES`: `GROUPS.length + j` for `TOPICS[j]`. Carried on
   *  the topic so a node's group is one store, never a scan of `TOPICS` per row. */
  readonly group: number;
  /** One entry per link end held by a node of this topic, plus one per node: drawing an
   *  entry uniformly is drawing a node in proportion to its links. */
  readonly ends: number[];
}

function topicsFor(count: number): Topic[] {
  const wanted = clamp(Math.round(Math.sqrt(count) / 2), 3, TOPICS.length);
  return TOPICS.slice(0, wanted).map((name, j) => ({ name, group: GROUPS.length + j, ends: [] }));
}

function vaultTarget(topic: Topic, index: number, rnd: () => number): number {
  const stay = rnd() < SAME_TOPIC;
  const roll = rnd();
  if (stay && topic.ends.length > 0) return topic.ends[pickIndex(topic.ends.length, roll)] ?? 0;
  return Math.floor(roll * index);
}

/** Draw order: per node (topic, kind), then per link (stay, target, kind). */
function vaultDraw(count: number, degree: number, rnd: () => number, planned: number): Drawn {
  const topics = topicsFor(count);
  const fallback = topics[0] ?? { name: "Layout", group: GROUPS.length, ends: [] };
  const nodes: IngestNode[] = [];
  const kinds = new Uint8Array(count);
  const groups = new Uint8Array(count);
  const counts = new Uint32Array(count);
  const from = new Uint32Array(planned);
  const to = new Uint32Array(planned);
  const edgeKinds = new Uint8Array(planned);
  /** The topic each row drew, so a link that lands on an earlier row can grow *that* topic's
   *  ends as well as its own. Without it the vault shape drifts: a later draw would stop
   *  seeing the link ends the earlier ones added. */
  const topicOf: Topic[] = [];
  let edgeCount = 0;
  for (let i = 0; i < count; i += 1) {
    const topic = topics[pickIndex(topics.length, rnd())] ?? fallback;
    const drawn = nodeKindIndex(rnd());
    const kind = topic.ends.length === 0 ? DATABASE_KIND : drawn;
    kinds[i] = kind;
    groups[i] = topic.group;
    nodes.push(nodeOf(i, kind, topic.group, `n-${i}`));
    for (let k = 0; k < degree && i > 0; k += 1) {
      const target = vaultTarget(topic, i, rnd);
      from[edgeCount] = i;
      to[edgeCount] = target;
      edgeKinds[edgeCount] = edgeKindIndex(rnd());
      edgeCount += 1;
      bump(counts, i);
      bump(counts, target);
      topicOf[target]?.ends.push(target);
      topic.ends.push(i);
    }
    topic.ends.push(i);
    topicOf.push(topic);
  }
  checkEdges("vault", edgeCount, planned);
  return { nodes, kinds, groups, from, to, edgeKinds, counts, edgeCount };
}

/** Draws `spec`. One call, one set of columns: both consumers read this. */
export function drawGraph(spec: SyntheticSpec): Drawn {
  const count = clamp(spec.nodeCount, 2, MAX_NODES);
  const degree = clamp(spec.degree, 0, MAX_DEGREE);
  const planned = plannedEdges(count, degree, spec.shape);
  const rnd = mulberry32(spec.seed);
  return spec.shape === "vault"
    ? vaultDraw(count, degree, rnd, planned)
    : randomDraw(count, degree, rnd, planned);
}

/** Weight is the node's link count over the graph's largest, so nodes size by connectivity.
 *  The counts arrive as node indices, filled in by the generator that made the edges: an id is
 *  never hashed, which at a million nodes was the whole cost of this pass. Shared by both
 *  consumers — the columns path reads the weights back off these nodes. */
export function applyDegreeWeights(nodes: IngestNode[], counts: Uint32Array): void {
  let max = 0;
  for (const count of counts) if (count > max) max = count;
  for (const [index, node] of nodes.entries()) {
    node.weight = max === 0 ? 0 : (counts[index] ?? 0) / max;
  }
}

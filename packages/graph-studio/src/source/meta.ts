/**
 * What the studio knows about each node, as columns in the motor's dense order. The
 * snapshot says where a node is; this says what it is called and how it is grouped.
 * Joined by id against the snapshot's own id table, never by position in the document.
 */
import type { IngestNode, NodeKind } from "./ingest.ts";

export interface GraphMeta {
  readonly nodeCount: number;
  readonly ids: readonly string[];
  readonly labels: readonly string[];
  readonly kinds: readonly NodeKind[];
  /** Distinct group names, first seen first. */
  readonly groups: readonly string[];
  /** Index into `groups` per node. */
  readonly group: Uint16Array;
  readonly weight: Float32Array;
  /** Links per node: a self-loop counts once, a parallel edge each time. */
  readonly degree: Uint32Array;
  readonly maxDegree: number;
}

export interface Ends {
  readonly source: Uint32Array;
  readonly target: Uint32Array;
}

export const UNGROUPED = "(no group)";
export const OTHER_GROUPS = "(other groups)";
/** `group` is a Uint16 column; past this many names the rest share one entry. */
const MAX_GROUPS = 4096;

/** The snapshot and the document do not describe the same nodes. */
export class MetaMismatch extends Error {
  constructor(message: string) {
    super(message);
    this.name = "MetaMismatch";
  }
}

function degreesOf(nodeCount: number, ends: Ends): Uint32Array {
  const degree = new Uint32Array(nodeCount);
  for (let e = 0; e < ends.source.length; e += 1) {
    const s = ends.source[e] ?? 0;
    const t = ends.target[e] ?? 0;
    degree[s] = (degree[s] ?? 0) + 1;
    if (t !== s) degree[t] = (degree[t] ?? 0) + 1;
  }
  return degree;
}

function groupIndex(groups: string[], slots: Map<string, number>, name: string): number {
  const known = slots.get(name);
  if (known !== undefined) return known;
  const slot = groups.length < MAX_GROUPS ? groups.length : MAX_GROUPS;
  if (slot === groups.length) groups.push(groups.length < MAX_GROUPS ? name : OTHER_GROUPS);
  slots.set(name, slot);
  return slot;
}

function inOrder(nodes: readonly IngestNode[], order: readonly string[]): IngestNode[] {
  if (order.length !== nodes.length) {
    throw new MetaMismatch(`the snapshot holds ${order.length} nodes and the document ${nodes.length}`);
  }
  const byId = new Map(nodes.map((node) => [node.id, node]));
  return order.map((id) => {
    const node = byId.get(id);
    if (node === undefined) throw new MetaMismatch(`the snapshot names node ${JSON.stringify(id)}, which the document does not hold`);
    return node;
  });
}

export function metaOf(nodes: readonly IngestNode[], order: readonly string[], ends: Ends): GraphMeta {
  const ordered = inOrder(nodes, order);
  const groups: string[] = [];
  const slots = new Map<string, number>();
  const group = new Uint16Array(ordered.length);
  const weight = new Float32Array(ordered.length);
  for (const [i, node] of ordered.entries()) {
    group[i] = groupIndex(groups, slots, node.group ?? UNGROUPED);
    weight[i] = node.weight;
  }
  const degree = degreesOf(ordered.length, ends);
  return {
    nodeCount: ordered.length,
    ids: order,
    labels: ordered.map((node) => node.label),
    kinds: ordered.map((node) => node.kind),
    groups, group, weight, degree,
    maxDegree: degree.reduce((max, value) => Math.max(max, value), 0),
  };
}

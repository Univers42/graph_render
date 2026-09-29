/**
 * Who touches whom, as CSR: built once per topology in O(n + m), read in O(degree).
 * A self-loop lists its node once; a parallel edge lists its neighbour once per edge.
 */

export interface Adjacency {
  readonly nodeCount: number;
  /** Node `i` owns entries `start[i]..start[i+1]`. */
  readonly start: Uint32Array;
  readonly neighbour: Uint32Array;
  /** The edge behind each entry. */
  readonly edge: Uint32Array;
}

interface Ends {
  readonly source: Uint32Array;
  readonly target: Uint32Array;
}

function countEntries(nodeCount: number, ends: Ends): Uint32Array {
  const start = new Uint32Array(nodeCount + 1);
  for (let e = 0; e < ends.source.length; e += 1) {
    const s = ends.source[e] ?? 0;
    const t = ends.target[e] ?? 0;
    start[s + 1] = (start[s + 1] ?? 0) + 1;
    if (t !== s) start[t + 1] = (start[t + 1] ?? 0) + 1;
  }
  for (let i = 0; i < nodeCount; i += 1) start[i + 1] = (start[i + 1] ?? 0) + (start[i] ?? 0);
  return start;
}

export function adjacencyOf(nodeCount: number, ends: Ends): Adjacency {
  const start = countEntries(nodeCount, ends);
  const next = start.slice(0, nodeCount);
  const size = start[nodeCount] ?? 0;
  const neighbour = new Uint32Array(size);
  const edge = new Uint32Array(size);
  const put = (node: number, other: number, e: number): void => {
    const at = next[node] ?? 0;
    neighbour[at] = other;
    edge[at] = e;
    next[node] = at + 1;
  };
  for (let e = 0; e < ends.source.length; e += 1) {
    const s = ends.source[e] ?? 0;
    const t = ends.target[e] ?? 0;
    put(s, t, e);
    if (t !== s) put(t, s, e);
  }
  return { nodeCount, start, neighbour, edge };
}

export function degreeOf(adjacency: Adjacency, node: number): number {
  return (adjacency.start[node + 1] ?? 0) - (adjacency.start[node] ?? 0);
}

/** Sets `mask[i] = 1` for `node` and every neighbour, 0 elsewhere. `node < 0` clears it. */
export function markNeighbourhood(adjacency: Adjacency, node: number, mask: Uint8Array): void {
  mask.fill(0);
  if (node < 0 || node >= adjacency.nodeCount) return;
  mask[node] = 1;
  const end = adjacency.start[node + 1] ?? 0;
  for (let at = adjacency.start[node] ?? 0; at < end; at += 1) mask[adjacency.neighbour[at] ?? 0] = 1;
}

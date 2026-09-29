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
  /** Direction of each entry: 0 = outgoing (source→target), 1 = incoming (target→source). */
  readonly direction: Uint8Array;
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
  const direction = new Uint8Array(size);
  const put = (node: number, other: number, e: number, dir: number): void => {
    const at = next[node] ?? 0;
    neighbour[at] = other;
    edge[at] = e;
    direction[at] = dir;
    next[node] = at + 1;
  };
  for (let e = 0; e < ends.source.length; e += 1) {
    const s = ends.source[e] ?? 0;
    const t = ends.target[e] ?? 0;
    put(s, t, e, 0);
    if (t !== s) put(t, s, e, 1);
  }
  return { nodeCount, start, neighbour, edge, direction };
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

/** Which entries a local-graph walk follows. All three false means "every edge, both ways". */
export interface WalkKinds {
  readonly incoming: boolean;
  readonly outgoing: boolean;
  readonly neighbours: boolean;
}

function followsEntry(kinds: WalkKinds, direction: number): boolean {
  if (kinds.neighbours || !(kinds.incoming || kinds.outgoing)) return true;
  return direction === 0 ? kinds.outgoing : kinds.incoming;
}

/** Marks the unvisited nodes `node` reaches along the kinds asked for, and queues them. */
function expand(adjacency: Adjacency, node: number, kinds: WalkKinds, seen: Uint8Array): number[] {
  const reached: number[] = [];
  const { start, neighbour, direction } = adjacency;
  for (let at = start[node] ?? 0; at < (start[node + 1] ?? 0); at += 1) {
    const other = neighbour[at] ?? 0;
    if (seen[other] === 1 || !followsEntry(kinds, direction[at] ?? 0)) continue;
    seen[other] = 1;
    reached.push(other);
  }
  return reached;
}

/**
 * The nodes within `depth` hops of `root`, as a 0/1 mask (root included). `depth` is 1..5, else
 * the mask is empty. `outgoing` follows source to target, `incoming` target to source,
 * `neighbours` follows both; none set means both. Level by level over the CSR: O(n + m).
 * Ponytail: on a graph with parallel edges of opposite direction the walk follows either one.
 */
export function bfsMask(adjacency: Adjacency, root: number, depth: number, kinds: WalkKinds): Uint8Array {
  const visited = new Uint8Array(adjacency.nodeCount);
  if (root < 0 || root >= adjacency.nodeCount || depth < 1 || depth > 5) return visited;
  visited[root] = 1;
  let frontier = [root];
  for (let level = 0; level < depth && frontier.length > 0; level += 1) {
    frontier = frontier.flatMap((node) => expand(adjacency, node, kinds, visited));
  }
  return visited;
}

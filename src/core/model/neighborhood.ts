/**
 * BFS the model's adjacency index to collect a node's neighborhood (the node plus
 * everything within `depth` hops). Used to drive focus-dimming when a node is
 * selected. Pure, O(reachable edges).
 */

import type { EdgeId, GraphModel, NodeId } from "../types";

/**
 * Core BFS. Shared by `neighborhood` (nodes only) and `neighborhoodEdges`
 * (nodes + the edges walked to reach them) so the two cannot diverge again —
 * they used to, which is how the edge set went missing from the public API.
 *
 * A `nodeId` that is not in the model yields an empty result rather than a
 * one-element set containing the phantom id: the previous seed-the-frontier
 * approach returned `Set { "<deleted id>" }` for a node that had just been
 * removed, which callers read as "something is selected and in focus".
 */
function bfs(
  model: GraphModel,
  id: NodeId,
  depth: number,
): { nodeIds: Set<NodeId>; edgeIds: Set<EdgeId> } {
  const nodeIds = new Set<NodeId>();
  const edgeIds = new Set<EdgeId>();
  if (!model.nodeById.has(id)) return { nodeIds, edgeIds };

  nodeIds.add(id);
  let frontier: NodeId[] = [id];
  for (let hop = 0; hop < depth; hop += 1) {
    const next: NodeId[] = [];
    for (const current of frontier) {
      for (const edgeId of model.adjacency.get(current) ?? []) {
        edgeIds.add(edgeId);
        const edge = model.edgeById.get(edgeId);
        if (!edge) continue;
        const other = edge.source === current ? edge.target : edge.source;
        if (!nodeIds.has(other)) {
          nodeIds.add(other);
          next.push(other);
        }
      }
    }
    frontier = next;
    // Nothing new was reached: further hops cannot reach anything either.
    if (frontier.length === 0) break;
  }
  return { nodeIds, edgeIds };
}

export function neighborhood(model: GraphModel, id: NodeId, depth = 1): Set<NodeId> {
  return bfs(model, id, depth).nodeIds;
}

/**
 * The same traversal, also returning the edges walked. A consumer that highlights
 * the edges of a focused node needs these: `neighborhood` alone cannot supply
 * them, so the host grew its own copy to get at the set.
 */
export function neighborhoodEdges(
  model: GraphModel,
  id: NodeId,
  depth = 1,
): { nodeIds: Set<NodeId>; edgeIds: Set<EdgeId> } {
  return bfs(model, id, depth);
}

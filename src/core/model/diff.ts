/**
 * Incremental diff between two models — the performance unlock.
 *
 * Because node/edge ids are deterministic, a single edit yields a tiny patch
 * (typically one `updatedNode`), which a layout worker and renderer can apply
 * instead of rebuilding the world. Set-difference over the id maps → O(N + E).
 *
 * This is the missing half of the package's own `GraphPatch` type. `core/types.ts`
 * declared the shape and `src/index.ts` re-exported it, but nothing produced one,
 * and the engine's data path is a full replace (`setModel` → `setGraph` +
 * `layout.rebuild`) with no incremental branch — so a declared public type with
 * no implementation is a contract that cannot be honoured. With this, a host can
 * at least compute a patch, and the type means something.
 *
 * Extracted verbatim from osionos `src/features/second-brain/model/diffGraph.ts`.
 * It consumes this package's own `nodesEqual`, so the `icon` fix in
 * `core/model/model.ts` applies here automatically: without that fix an
 * icon-only edit diffed to an empty patch.
 */

import type { GraphEdge, GraphModel, GraphPatch } from "../types";
import { nodesEqual } from "./model";

/**
 * Value-equality for diffing an edge whose id survived.
 *
 * The mirror of `nodesEqual`, and needed for the same reason: an edge id is
 * content-addressed over `endpoints:kind:label` only, so `strength` and
 * `recordId` live entirely outside it. Without this, changing an edge's strength
 * produces an empty patch.
 */
export function edgesEqual(a: GraphEdge, b: GraphEdge): boolean {
  return (
    a.source === b.source &&
    a.target === b.target &&
    a.kind === b.kind &&
    a.label === b.label &&
    a.strength === b.strength &&
    a.directed === b.directed &&
    a.recordId === b.recordId
  );
}

export function diffGraph(previous: GraphModel, next: GraphModel): GraphPatch {
  const addedNodes = [];
  const updatedNodes = [];
  for (const node of next.nodeById.values()) {
    const before = previous.nodeById.get(node.id);
    if (!before) addedNodes.push(node);
    else if (!nodesEqual(before, node)) updatedNodes.push(node);
  }

  const removedNodeIds = [];
  for (const id of previous.nodeById.keys()) {
    if (!next.nodeById.has(id)) removedNodeIds.push(id);
  }

  const addedEdges = [];
  const updatedEdges = [];
  for (const edge of next.edgeById.values()) {
    const before = previous.edgeById.get(edge.id);
    if (!before) addedEdges.push(edge);
    else if (!edgesEqual(before, edge)) updatedEdges.push(edge);
  }

  const removedEdgeIds = [];
  for (const id of previous.edgeById.keys()) {
    if (!next.edgeById.has(id)) removedEdgeIds.push(id);
  }

  return { addedNodes, updatedNodes, removedNodeIds, addedEdges, updatedEdges, removedEdgeIds };
}

/** True when a patch carries no changes (skip worker/render work entirely). */
export function isEmptyPatch(patch: GraphPatch): boolean {
  return (
    patch.addedNodes.length === 0 &&
    patch.updatedNodes.length === 0 &&
    patch.removedNodeIds.length === 0 &&
    patch.addedEdges.length === 0 &&
    patch.updatedEdges.length === 0 &&
    patch.removedEdgeIds.length === 0
  );
}

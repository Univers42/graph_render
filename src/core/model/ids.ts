/**
 * Deterministic identity helpers. Node ids mirror the BaaS coordinate scheme so
 * the same record keeps its id (and therefore its layout position) across
 * rebuilds; edge ids are content-addressed so A→B and B→A collapse when
 * undirected.
 */

import type { EdgeId, EdgeKind, NodeId } from "../types";

/** Build a record node id from the BaaS-style coordinates. */
export function makeRecordNodeId(source: string, databaseId: string, recordId: string): NodeId {
  return `${source}:${databaseId}:${recordId}`;
}

/** Build a (free or overlay) note node id. */
export function makeNoteNodeId(noteId: string): NodeId {
  return `note:${noteId}`;
}

/** Build a synthetic tag-hub node id. */
export function makeTagNodeId(tagValue: string): NodeId {
  return `tag:${tagValue}`;
}

/** The parts of a record node id, as produced by `parseNodeId`. */
export interface RecordRef {
  mount: string;
  resource: string;
  pk: string;
}

/**
 * Split a record node id back into the coordinates `makeRecordNodeId` joined.
 * The inverse of the write half above, which the package previously lacked: hosts
 * had to hand-roll a splitter (and one of them re-implemented `makeRecordNodeId` a
 * third time while doing it). `pk` may itself contain `:`, so everything after the
 * second segment rejoins.
 */
export function parseNodeId(nodeId: string): RecordRef {
  const [mount = "", resource = "", ...rest] = nodeId.split(":");
  return { mount, resource, pk: rest.join(":") };
}

/**
 * Deterministic edge id. Directed edges keep their orientation; undirected ones
 * sort their endpoints so A–B and B–A collapse to a single edge.
 */
export function makeEdgeId(
  source: NodeId,
  target: NodeId,
  kind: EdgeKind,
  label: string,
  directed: boolean,
): EdgeId {
  const endpoints = directed
    ? `${source}->${target}`
    : [source, target].sort((a, b) => a.localeCompare(b)).join("--");
  return `${endpoints}:${kind}:${label}`;
}

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

/**
 * The coordinates of a record node id, as produced by `parseNodeId`.
 *
 * Field names deliberately match `makeRecordNodeId`'s parameters — `source`,
 * `databaseId`, `recordId` — so the round trip reads as one grammar. An earlier
 * draft returned `{ mount, resource, pk }`, the host's BaaS wire vocabulary, which
 * meant `parseNodeId(id).databaseId` was `undefined` on a public type. Same three
 * values, two names for each, in one file.
 */
export interface RecordRef {
  source: string;
  databaseId: string;
  recordId: string;
}

/** The two id prefixes `makeRecordNodeId` never produces. */
const NOTE_PREFIX = "note:";
const TAG_PREFIX = "tag:";

/**
 * Split a record node id back into the coordinates `makeRecordNodeId` joined.
 * The inverse of the write half above, which the package previously lacked: hosts
 * had to hand-roll a splitter (and one of them re-implemented `makeRecordNodeId` a
 * third time while doing it).
 *
 * Returns `null` for anything that is not a record node id — currently `note:` and
 * `tag:` ids, which `makeNoteNodeId` / `makeTagNodeId` three lines either side of
 * this function manufacture. The previous version returned a non-nullable
 * `RecordRef` for those, so `parseNodeId("tag:vintage")` confidently reported
 * `{ source: "tag", databaseId: "vintage", recordId: "" }` — a populated-looking
 * answer to a question that has none. A host filtering records by resource would
 * silently test a field that means something else.
 *
 * `recordId` may itself contain `:` (composite keys), so everything after the
 * second segment rejoins.
 *
 * PONYTAIL: `source` and `databaseId` containing `:` cannot be represented — the
 * grammar is ambiguous, and no amount of parsing recovers which colon was the
 * separator. `makeRecordNodeId("my:db", …)` therefore produces an id that does not
 * round-trip, and the parse returns a shifted, wrong result rather than `null`.
 * The fix is a caller-side constraint (reject `:` in those coordinates), not
 * something this function can detect.
 */
export function parseNodeId(nodeId: string): RecordRef | null {
  if (nodeId.startsWith(NOTE_PREFIX) || nodeId.startsWith(TAG_PREFIX)) return null;
  const [source = "", databaseId = "", ...rest] = nodeId.split(":");
  return { source, databaseId, recordId: rest.join(":") };
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

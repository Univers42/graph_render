/**
 * The verbs a host calls on `<graph-studio>` (`docs/contract/host-api.md`, Surface). Each one
 * checks what the host passed, then goes through the registry like a click or a typed command,
 * so arguments are refused in one place and every call is in the log.
 */
import { distinctIds, nodesWithIds } from "../actions/nodes.ts";
import type { LogEntry } from "../state/model.ts";
import { firstOf } from "../studio/pipeline.ts";
import type { Studio } from "../studio/studio.ts";
import { type LoadResult, wireError } from "./contract.ts";

export interface HostVerbs {
  loadGraph(doc: unknown): Promise<LoadResult>;
  focusNode(id: unknown): boolean;
  selectNodes(ids: unknown): boolean;
  selectedIds(): readonly string[];
}

/** The name a host reads on a refused `loadGraph`: the `graph-error` code for the same failure. */
function refusal(entry: LogEntry): Error {
  const error = new Error(entry.error?.detail ?? entry.message);
  error.name = entry.error === null ? "Error" : wireError(entry.error);
  return error;
}

/**
 * WHY the host's object is serialised here and not posted as it is: the worker reads text, and
 * the one normaliser (`source/ingest.ts`) then refuses a bad document exactly as it refuses a
 * dropped file. A value `JSON.stringify` cannot write is the host's mistake, refused before the
 * studio sees it.
 */
function documentText(doc: unknown): string {
  if (typeof doc !== "object" || doc === null) throw new TypeError("loadGraph takes an ingest document, an object");
  const text: unknown = JSON.stringify(doc);
  if (typeof text !== "string") throw new TypeError("loadGraph: the document serialises to nothing");
  return text;
}

async function loadGraph(studio: Studio, started: Promise<unknown>, doc: unknown): Promise<LoadResult> {
  const text = documentText(doc);
  await started;
  const entry = await studio.dispatch("source.host", { name: "host", text });
  const { graph } = studio.store.get();
  if (!entry.ok || graph === null) throw refusal(entry);
  return Object.freeze({ nodes: graph.nodeCount, edges: graph.edgeCount, notes: Object.freeze([...firstOf(graph.notes)]) });
}

/**
 * Exact ids in the frame on screen, checked before anything moves (verdict 2): an id that is in
 * no node answers false and leaves the camera and the selection as they were.
 */
function focusNode(studio: Studio, id: unknown): boolean {
  const { meta } = studio.store.get();
  if (typeof id !== "string" || meta === null || !meta.ids.includes(id)) return false;
  void studio.dispatch("node.focus", { id });
  return true;
}

function sameSet(nodes: readonly number[], held: readonly number[]): boolean {
  if (nodes.length !== held.length) return false;
  const kept = new Set(held);
  return nodes.every((node) => kept.has(node));
}

/** False, and nothing moves, unless every id is a node of the frame on screen. */
function selectNodes(studio: Studio, ids: unknown): boolean {
  const { meta, selection } = studio.store.get();
  const wanted = distinctIds(ids);
  if (wanted === null || meta === null) return false;
  const nodes = nodesWithIds(meta, wanted);
  if (nodes === null) return false;
  // The set the reader already has: no command, so no `node-select` for a change that is none.
  if (sameSet(nodes, selection)) return true;
  void studio.dispatch("node.select", { ids: JSON.stringify(wanted) });
  return true;
}

function selectedIds(studio: Studio): readonly string[] {
  const { meta, selection } = studio.store.get();
  if (meta === null) return Object.freeze([]);
  return Object.freeze(selection.map((node) => meta.ids[node] ?? ""));
}

export function hostVerbs(studio: Studio, started: Promise<unknown>): HostVerbs {
  return {
    loadGraph: (doc) => loadGraph(studio, started, doc),
    focusNode: (id) => focusNode(studio, id),
    selectNodes: (ids) => selectNodes(studio, ids),
    selectedIds: () => selectedIds(studio),
  };
}

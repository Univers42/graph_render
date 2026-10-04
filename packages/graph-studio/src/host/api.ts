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
import { emit } from "./events.ts";

export interface HostVerbs {
  loadGraph(doc: unknown): Promise<LoadResult>;
  focusNode(id: unknown): Promise<boolean>;
  selectNodes(ids: unknown): Promise<boolean>;
  selectedIds(): readonly string[];
}

/** The name a host reads on a refused `loadGraph`: the `graph-error` code for the same failure. */
function refusal(entry: LogEntry): Error {
  const error = new Error(entry.error?.detail ?? entry.message);
  error.name = entry.error === null ? "Error" : wireError(entry.error);
  return error;
}

/** The name the ingest refusals carry, and the one a host reads on a document it cannot write. */
const INGEST_REFUSAL = "IngestRefusal";

/**
 * A document the studio cannot write is refused as one it cannot read: the same name the ingest
 * refusal carries, and the same name its `graph-error` says, whatever `JSON.stringify` threw.
 */
function unserialisable(error: unknown): Error {
  const refused = new Error(`loadGraph could not write the document: ${error instanceof Error ? error.message : String(error)}`);
  refused.name = INGEST_REFUSAL;
  return refused;
}

/**
 * WHY the host's object is serialised here and not posted as it is: the worker reads text, and
 * the one normaliser (`source/ingest.ts`) then refuses a bad document exactly as it refuses a
 * dropped file. A value `JSON.stringify` cannot write is the host's mistake, refused before the
 * studio sees it — and refused as an ingest refusal, because a `RangeError` from a `toJSON` or
 * from a document past V8's string ceiling is a document refused, not a studio bug.
 */
function documentText(doc: unknown): string {
  if (typeof doc !== "object" || doc === null) throw new TypeError("loadGraph takes an ingest document, an object");
  let text: unknown;
  try {
    text = JSON.stringify(doc);
  } catch (error) {
    throw unserialisable(error);
  }
  if (typeof text !== "string") throw new TypeError("loadGraph: the document serialises to nothing");
  return text;
}

async function loadGraph(host: EventTarget, studio: Studio, started: Promise<unknown>, doc: unknown): Promise<LoadResult> {
  let text: string;
  try {
    text = documentText(doc);
  } catch (error) {
    // The studio never saw the document, so nothing set `state.error` and nothing would be sent.
    // Only an ingest refusal is announced: a `TypeError` for a value that is not a document is the
    // host's own mistake at the call boundary, and has never emitted a `graph-error`.
    if (error instanceof Error && error.name === INGEST_REFUSAL) {
      emit(host, "graph-error", { error: error.name, message: error.message });
    }
    throw error;
  }
  await started;
  const entry = await studio.dispatch("source.host", { name: "host", text });
  const { graph } = studio.store.get();
  if (!entry.ok || graph === null) throw refusal(entry);
  return Object.freeze({ nodes: graph.nodeCount, edges: graph.edgeCount, notes: Object.freeze([...firstOf(graph.notes)]) });
}

/**
 * Exact ids in the frame on screen, checked before anything moves (verdict 2): an id that is in
 * no node answers false and leaves the camera and the selection as they were.
 *
 * WHY the answer waits for the command: a `loadGraph` landing between the check and the dispatch
 * replaces the frame, and a `true` handed back then would be a claim about a graph that is no
 * longer on screen. While the frame is the one the check was made against the answer stands on
 * its own; when it has been replaced, only what the command says counts.
 */
async function focusNode(studio: Studio, id: unknown): Promise<boolean> {
  const { meta } = studio.store.get();
  if (typeof id !== "string" || meta === null || !meta.ids.includes(id)) return false;
  const entry = await studio.dispatch("node.focus", { id });
  const now = studio.store.get().meta;
  if (now === meta) return true;
  return entry.ok && now !== null && now.ids.includes(id);
}

function sameSet(nodes: readonly number[], held: readonly number[]): boolean {
  if (nodes.length !== held.length) return false;
  const kept = new Set(held);
  return nodes.every((node) => kept.has(node));
}

/** False, and nothing moves, unless every id is a node of the frame on screen. */
async function selectNodes(studio: Studio, ids: unknown): Promise<boolean> {
  const { meta, selection } = studio.store.get();
  const wanted = distinctIds(ids);
  if (wanted === null || meta === null) return false;
  const nodes = nodesWithIds(meta, wanted);
  if (nodes === null) return false;
  // The set the reader already has: no command, so no `node-select` for a change that is none.
  if (sameSet(nodes, selection)) return true;
  const entry = await studio.dispatch("node.select", { ids: JSON.stringify(wanted) });
  const { meta: now, selection: held } = studio.store.get();
  if (now === meta) return true;
  return entry.ok && now !== null && sameSet(nodesWithIds(now, wanted) ?? [], held);
}

function selectedIds(studio: Studio): readonly string[] {
  const { meta, selection } = studio.store.get();
  if (meta === null) return Object.freeze([]);
  return Object.freeze(selection.map((node) => meta.ids[node] ?? ""));
}

export function hostVerbs(host: EventTarget, studio: Studio, started: Promise<unknown>): HostVerbs {
  return {
    loadGraph: (doc) => loadGraph(host, studio, started, doc),
    focusNode: (id) => focusNode(studio, id),
    selectNodes: (ids) => selectNodes(studio, ids),
    selectedIds: () => selectedIds(studio),
  };
}
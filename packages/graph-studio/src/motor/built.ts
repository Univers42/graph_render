/**
 * The graph a session holds between runs, and the description of it a snapshot's node order
 * asks for. Split from `session.ts`, which owns the runs over it.
 */
import { decodeSnapshot, idAt } from "../../../graph-render/src/snapshot/decode.ts";
import type { IngestNode } from "../source/ingest.ts";
import { type GraphMeta, metaOf } from "../source/meta.ts";
import type { ForceEngine, ForceKnobs, ForcePort, Growable, LiveForce } from "./live.ts";

export interface Built<Handle> {
  readonly handle: Handle;
  /** The graph's nodes as the studio knows them; an extend appends to it (`metaOf` reads this). */
  nodes: readonly IngestNode[];
  /** The id table the description was last built against; `null` before the first run. */
  described: Uint8Array | null;
  /** The motor's live session over this graph, made when one is first asked for. */
  forced: (ForcePort & Growable<Handle>) | null;
  /** The port over it, cached so the loop sees one object for one session. */
  port: LiveForce | null;
  /** Node ids in the force session's dense row order; `null` until a layout has run. */
  order: readonly string[] | null;
  /** The tick the live session is made with, set by each run. */
  engine: ForceEngine;
  /** True when the last run drew a picture the session starts from, false after a scatter. */
  warm: boolean;
  /** The knobs the last session ran with, so a re-layout keeps what the user tuned. */
  knobs: ForceKnobs;
}

function sameBytes(a: Uint8Array, b: Uint8Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i += 1) if (a[i] !== b[i]) return false;
  return true;
}

/** The description of the graph in the order this snapshot uses, if that order is news. */
export function describe<Handle>(built: Built<Handle>, bytes: Uint8Array): GraphMeta | null {
  const snapshot = decodeSnapshot(bytes);
  const table = snapshot.nodeIds.bytes;
  if (built.described !== null && sameBytes(built.described, table)) return null;
  const order = Array.from({ length: snapshot.nodeCount }, (_, i) => idAt(snapshot.nodeIds, i));
  built.described = table.slice();
  built.order = order;
  return metaOf(built.nodes, order, snapshot);
}

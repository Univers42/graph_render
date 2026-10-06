/**
 * A newly loaded document replacing the one on screen. It always fades from the old picture, and
 * where the two share node ids those nodes move from where they were drawn instead of jumping:
 * old-only nodes fade out, new-only ones fade in, shared ones slide (FrameOptions.start).
 *
 * Caveat: the start is the old frame's settled position, so a switch made while the old drawing
 * was still easing jumps those nodes to where they were heading. Past CARRY_MAX nodes on either
 * side nothing is carried and the switch only fades: matching builds one string per node, and at a
 * million nodes that is a pause the fade does not have. Ids are matched exactly, so a document
 * that renamed its nodes fades and does not move.
 */
import type { Frame } from "../../../graph-render/src/frame.ts";
import { type Snapshot, type StringTable, decodeSnapshot } from "../../../graph-render/src/snapshot/decode.ts";
import type { View } from "../../../graph-render/src/view.ts";
import type { Held } from "./pipeline/clear.ts";

export const CARRY_MAX = 262_144;

const utf8 = new TextDecoder();

function idAt(table: StringTable, at: number): string {
  return utf8.decode(table.bytes.subarray(table.offsets[at] ?? 0, table.offsets[at + 1] ?? 0));
}

/** Where each node of `next` was drawn in `held`, NaN where it was not; null when no node is shared. */
export function carriedStart(held: Held | null, next: Snapshot): { x: Float32Array; y: Float32Array } | null {
  if (held === null || held.ends.nodeCount === 0 || held.ends.nodeCount > CARRY_MAX || next.nodeIds.count > CARRY_MAX) return null;
  const old = decodeSnapshot(held.bytes).nodeIds;
  const index = new Map<string, number>();
  for (let at = 0; at < old.count; at += 1) index.set(idAt(old, at), at);
  const x = new Float32Array(next.nodeIds.count).fill(Number.NaN);
  const y = new Float32Array(next.nodeIds.count).fill(Number.NaN);
  let shared = 0;
  for (let at = 0; at < next.nodeIds.count; at += 1) {
    const from = index.get(idAt(next.nodeIds, at));
    if (from === undefined) continue;
    x[at] = held.ends.x[from] ?? Number.NaN;
    y[at] = held.ends.y[from] ?? Number.NaN;
    shared += 1;
  }
  return shared === 0 ? null : { x, y };
}

/** Puts a fresh document's frame on the view, carried from `held` (the drawing it replaces) where they share nodes. */
export function showFresh(view: Pick<View, "crossFade" | "setFrame">, held: Held | null, next: Snapshot, frame: Frame): void {
  const start = carriedStart(held, next);
  view.crossFade();
  view.setFrame(frame, start === null ? { animate: false } : { animate: true, start });
}

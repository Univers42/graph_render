// Column views (`docs/contract/wasm-abi.md` "Zero-copy", C3/C7/C8/C10/C11): typed-array
// windows directly over the motor's own `WebAssembly.Memory`, never a copy. Two things
// make that safe to hand out at all:
//
//  1. Presence is decided from the run's node/edge geometry *kind*, the same table
//     `crates/graph-wasm/src/views.rs::column` matches on — never from `ptr === 0`, which
//     is not an "absent" signal: an empty-but-present column also reads (0, 0) — C3.
//  2. Every returned view is re-derived, never reused, once the motor's `epoch` has moved
//     past the one it was cached at (`bump()`, called on every mutating export) — a stale
//     view is refused a reuse even if its pointer, length and backing buffer still happen
//     to match (C10), and a view whose backing `ArrayBuffer` was replaced by a
//     `memory.grow` is always rebuilt, since `cached.view.buffer === buffer` alone would
//     already catch that even without the epoch.

import type { RawExports } from "./wasm.ts";
import { ColumnId, type Column, type Dim, type EdgeGeometryKind, type Handle, type NodeGeometryKind } from "./types.ts";
import { InvalidHandleError } from "./errors.ts";
import { decoder, frame, snapshotPtr } from "./calls.ts";
import { readKinds, type GeometryKinds } from "./geometry-kinds.ts";

// `NodeZ` is in this set and not left to the Uint32Array fallthrough below: without it the
// motor's f32 depths would be read as u32 words, which is the SDK's silent-mislabel bug
// (F2 in `docs/decisions/contract-3d.md`).
const F32_COLUMNS: ReadonlySet<number> = new Set([
  ColumnId.NodeX,
  ColumnId.NodeY,
  ColumnId.NodeR,
  ColumnId.NodeW,
  ColumnId.NodeH,
  ColumnId.NodeZ,
  ColumnId.EdgePts,
]);

/** `crates/graph-wasm/src/views.rs::node_column`'s presence table. `z` keys on `dim`, not
 *  on the node kind: a 3D snapshot's nodes are whatever kind they were, and every kind
 *  carries a z column. */
function nodeColumnApplies(nodeKind: NodeGeometryKind, columnId: number, dim: Dim): boolean {
  switch (columnId) {
    case ColumnId.NodeX:
    case ColumnId.NodeY:
      return true;
    case ColumnId.NodeR:
      return nodeKind === "Circle";
    case ColumnId.NodeW:
    case ColumnId.NodeH:
      return nodeKind === "Box";
    case ColumnId.NodeZ:
      return dim === 1;
    default:
      return false;
  }
}

/** `crates/graph-wasm/src/views.rs::edge_column`'s presence table. */
function edgeColumnApplies(edgeKind: EdgeGeometryKind, columnId: number): boolean {
  switch (columnId) {
    case ColumnId.EdgeSource:
    case ColumnId.EdgeTarget:
      return true;
    case ColumnId.EdgeOffsets:
    case ColumnId.EdgePts:
      return edgeKind === "Polyline" || edgeKind === "Curve";
    case ColumnId.EdgeCurveDegree:
      return edgeKind === "Curve";
    default:
      return false;
  }
}

/** Whether `columnId` exists at all for a snapshot of this node/edge kind and dimension —
 * the reserved ids (`NoteCode`/`NoteIndex`, Phase 3's `note.code`/`note.index`) never do,
 * whatever the kind. `dim` defaults to 0, so a caller that has not read it yet gets the
 * 2D table: absent, never a wrong column. */
export function columnApplies(
  nodeKind: NodeGeometryKind,
  edgeKind: EdgeGeometryKind,
  columnId: number,
  dim: Dim = 0,
): boolean {
  if (columnId === ColumnId.NoteCode || columnId === ColumnId.NoteIndex) return false;
  return nodeColumnApplies(nodeKind, columnId, dim) || edgeColumnApplies(edgeKind, columnId);
}

interface CacheEntry {
  epoch: number;
  ptr: number;
  len: number;
  view: Float32Array | Uint32Array;
}

export class ColumnViews {
  readonly #exports: RawExports;
  #epoch = 0;
  readonly #cache = new Map<string, CacheEntry>();
  /// What each handle's last successful run produced. Here rather than on `Motor`
  /// because every reader of it is here too, and a second `Motor` over one module must not
  /// share it: the kinds belong to a run, and a run belongs to one motor's handle.
  readonly #kinds = new Map<Handle, GeometryKinds>();

  constructor(exports: RawExports) {
    this.#exports = exports;
  }

  /** The current invalidation epoch. Exposed read-only so a caller — and
   * `harness/wasm-run.mjs --assert-zero-copy` — can observe that a motor call really did
   * move it forward, not just trust that it did (C10). */
  get epoch(): number {
    return this.#epoch;
  }

  /** Every mutating export calls this once, right before returning to its own caller —
   * `index.ts` is the only caller of this method. */
  bump(): void {
    this.#epoch += 1;
  }

  /** The column `columnId` of `handle`'s last run, given that run's own node/edge kind, or
   * `null` if `columnId` is reserved or does not apply to this geometry (C3). Zero-copy:
   * re-derived whenever the epoch, the pointer, the length or the backing buffer itself
   * has moved on since the last time this exact `(handle, columnId)` was fetched — never
   * reused across a call this SDK cannot prove did not change it (C7, C10).
   *
   * Ponytail: the cache below is only as good as `bump()` being called on every export
   * that can move or grow wasm memory. A view fetched, then held across a motor call that
   * allocates, then read without going through this method again reads **silent garbage
   * (or a detached buffer), never a thrown error** — the dangerous direction, because it
   * looks like data. Escape hatch: never hold a `Column` past the next call on this
   * `Motor` (any handle) — re-derive it via `Motor#column` after every call, which this
   * cache then serves for free when nothing actually moved. */
  get(handle: Handle, columnId: ColumnId): Column {
    const { nodeKind, edgeKind, dim } = this.kindsOf(handle);
    if (!columnApplies(nodeKind, edgeKind, columnId, dim)) return null;
    const ptr = this.#exports.gm_column_ptr(handle, columnId);
    const len = this.#exports.gm_column_len(handle, columnId);
    const key = `${handle}:${columnId}`;
    const buffer = this.#exports.memory.buffer;
    const cached = this.#cache.get(key);
    if (cached && cached.epoch === this.#epoch && cached.ptr === ptr && cached.len === len && cached.view.buffer === buffer) {
      return cached.view;
    }
    const view = F32_COLUMNS.has(columnId) ? new Float32Array(buffer, ptr, len) : new Uint32Array(buffer, ptr, len);
    this.#cache.set(key, { epoch: this.#epoch, ptr, len, view });
    return view;
  }

  /** The geometry kinds `handle`'s last successful run produced, or the refusal: a handle
   *  with no successful run yet is refused, not read as "every column absent" (C3). */
  kindsOf(handle: Handle): GeometryKinds {
    const kinds = this.#kinds.get(handle);
    if (kinds === undefined) throw new InvalidHandleError(`handle ${handle} has no successful run yet`);
    return kinds;
  }

  /** Records what the run that just succeeded produced for `handle`, so a later `get`,
   *  `toJSON` or `toBytes` knows which columns apply (C3). `what` names the run in a
   *  refusal, and is the id that was asked for. */
  record(handle: Handle, what: string): GeometryKinds {
    const kinds = readKinds(this.#exports, handle, what);
    this.#kinds.set(handle, kinds);
    return kinds;
  }

  /** The canonical JSON face of `handle`'s last run. Refuses with a
   *  `TamperedGeometryError` if a column view wrote a non-finite value into the motor's own
   *  buffers since that run (D9 re-validation, C8). */
  toJSON(handle: Handle): string {
    return decoder.decode(frame(this.#exports, snapshotPtr(this.#exports, "gm_snapshot_json", handle)));
  }

  /** The binary face of `handle`'s last run, under the same D9 re-validation as
   *  {@link ColumnViews.toJSON}. */
  toBytes(handle: Handle): Uint8Array {
    return frame(this.#exports, snapshotPtr(this.#exports, "gm_snapshot_bytes", handle));
  }

  /** Drops every cached view for `handle`, and the kinds its last run produced
   *  (`gm_release`, C6: its id is never reissued, so nothing should ever read a view under
   *  it again). This frees the SDK's own state only; the motor's memory is the wasm
   *  module's business. */
  forget(handle: Handle): void {
    this.#kinds.delete(handle);
    const prefix = `${handle}:`;
    for (const key of this.#cache.keys()) {
      if (key.startsWith(prefix)) this.#cache.delete(key);
    }
  }
}

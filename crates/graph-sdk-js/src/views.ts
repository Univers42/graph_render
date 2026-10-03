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

import { invoke } from "./calls.ts";
import { AbiContractError } from "./errors.ts";
import type { RawExports } from "./wasm.ts";
import { ColumnId, type Column, type Dim, type EdgeGeometryKind, type Handle, type NodeGeometryKind } from "./types.ts";

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
 * whatever the kind, and neither table below names them, so this needs no arm of its own.
 * `dim` defaults to 0, so a caller that has not read it yet gets the 2D table: absent,
 * never a wrong column. */
export function columnApplies(
  nodeKind: NodeGeometryKind,
  edgeKind: EdgeGeometryKind,
  columnId: number,
  dim: Dim = 0,
): boolean {
  return nodeColumnApplies(nodeKind, columnId, dim) || edgeColumnApplies(edgeKind, columnId);
}

/** Every id `ColumnId` registers, so `Motor#column` can refuse one this ABI never issued
 * instead of letting `WebAssembly`'s own `ToInt32` coercion pick a neighbour: `{}` and
 * `1e9` both arrived as column 0 and were served as `NODE_X`, silently. */
const REGISTERED = new Set<number>(Object.values(ColumnId));

/** Whether `columnId` is an id this ABI registers at all. Membership in a set, not a range
 * test: the ids are append-only and not promised contiguous, so a future id 14 with no 13
 * must not be readable through the gap. */
export function isRegisteredColumn(columnId: number): boolean {
  return REGISTERED.has(columnId);
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
  get(
    handle: Handle,
    columnId: ColumnId,
    nodeKind: NodeGeometryKind,
    edgeKind: EdgeGeometryKind,
    dim: Dim = 0,
  ): Column {
    if (!columnApplies(nodeKind, edgeKind, columnId, dim)) return null;
    // Through `invoke`, unlike every other `Motor` export: these two were the only ones
    // that skipped it, so a trapping module raised a raw `WebAssembly.RuntimeError` right
    // here, against the typed-error policy stated at the top of this file.
    const ptr = invoke("gm_column_ptr", () => this.#exports.gm_column_ptr(handle, columnId));
    const len = invoke("gm_column_len", () => this.#exports.gm_column_len(handle, columnId));
    const key = `${handle}:${columnId}`;
    const buffer = this.#exports.memory.buffer;
    const cached = this.#cache.get(key);
    if (cached && cached.epoch === this.#epoch && cached.ptr === ptr && cached.len === len && cached.view.buffer === buffer) {
      return cached.view;
    }
    const view = this.#view(ptr, len, columnId);
    this.#cache.set(key, { epoch: this.#epoch, ptr, len, view });
    return view;
  }

  /** The typed array over `(ptr, len)`, or a refusal when the pair is illegal by contract.
   *
   *  `columnApplies` above is about *kind*: it says this run should have this column id. It
   *  says nothing about the pair the module answered with, and `new Float32Array(buffer, 0, 4)`
   *  is not an error — it is a four-element window over the module's own magic and version
   *  words, handed back to the caller labelled `NODE_X`. So the three illegal pairs are
   *  refused here, in one place, the way `ForceSession#column` already refuses its own:
   *
   *  - `ptr === 0 && len > 0`. `(0, 0)` is *present but empty* and legal
   *    (`docs/contract/wasm-abi.md` "Columns"), which is exactly why `ptr === 0` is not a
   *    presence test: the pair is what is illegal, never the pointer alone.
   *  - `ptr % 4 !== 0`. Every column is `f32` or `u32`, so 4 is the element alignment, and
   *    the typed-array constructor would throw a bare `RangeError` instead of refusing.
   *  - `ptr + len * per > buffer.byteLength`. The window would run past the end. */
  #view(ptr: number, len: number, columnId: ColumnId): Float32Array | Uint32Array {
    const f32 = F32_COLUMNS.has(columnId);
    const per = f32 ? Float32Array.BYTES_PER_ELEMENT : Uint32Array.BYTES_PER_ELEMENT;
    const buffer = this.#exports.memory.buffer;
    const named = `column ${String(columnId)}`;
    if (ptr === 0 && len > 0) {
      throw new AbiContractError(`${named} answered (0, ${String(len)}): a present-but-empty column is (0, 0), and 0 is never a column's address`);
    }
    if (ptr % 4 !== 0) {
      throw new AbiContractError(`${named} answered pointer ${String(ptr)}, which is not 4-aligned`);
    }
    if (ptr + len * per > buffer.byteLength) {
      throw new AbiContractError(`${named} answered (${String(ptr)}, ${String(len)}), past the end of ${String(buffer.byteLength)} bytes`);
    }
    return f32 ? new Float32Array(buffer, ptr, len) : new Uint32Array(buffer, ptr, len);
  }

  /** Drops every cached view for `handle` (`gm_release`, C6: its id is never reissued, so
   * nothing should ever read a view under it again — this only frees the SDK's own cache
   * entries, the motor's memory is the wasm module's business). */
  forget(handle: Handle): void {
    const prefix = `${handle}:`;
    for (const key of this.#cache.keys()) {
      if (key.startsWith(prefix)) this.#cache.delete(key);
    }
  }
}

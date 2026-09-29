// The raw-call helpers every `Motor` method shares: trap wrapping, the error slot, framed
// buffers and registry scans. Pure functions over a module's `RawExports`, so `index.ts`
// keeps only the typed surface.

import { toU32, type RawExports } from "./wasm.ts";
import { MotorTrapError, RunRefusedError, TamperedGeometryError, codeName } from "./errors.ts";
import type { Handle } from "./types.ts";
import type { ColumnViews } from "./views.ts";

export const TAMPERED_GEOMETRY_CODE = 9; // Code::TamperedGeometry, crates/graph-wasm/src/errors.rs
export const INVALID_HANDLE_CODE = 1; // Code::InvalidHandle
export const NO_GEOMETRY_CODE = 10; // Code::NoGeometryYet

export const encoder = new TextEncoder();
export const decoder = new TextDecoder("utf-8", { fatal: true });

/** A loaded module and its view bookkeeping: what a non-degraded `Motor` holds. */
export interface Loaded {
  exports: RawExports;
  views: ColumnViews;
}

export function invoke<T>(name: string, fn: () => T): T {
  try {
    return fn();
  } catch (error) {
    if (error instanceof WebAssembly.RuntimeError) throw new MotorTrapError(name, error);
    throw error;
  }
}

export function lastError(exports: RawExports): number {
  return invoke("gm_last_error", () => exports.gm_last_error());
}

/** `[len: u32 LE][len bytes]` at `ptr`, copied out (`docs/contract/wasm-abi.md`
 * "Framed buffers"): this one buffer is reused by the next motor call, so nothing may
 * hold a reference into it past this method returning. */
export function frame(exports: RawExports, ptr: number): Uint8Array {
  const header = new DataView(exports.memory.buffer);
  const len = header.getUint32(ptr, true);
  return new Uint8Array(exports.memory.buffer, ptr + 4, len).slice();
}

/** Scans one registry the way every registry in this ABI is discovered: the count,
 *  then a framed id per index. Written once so the three cannot drift into three
 *  differently-shaped scans (C1). A `0` from the id export is the motor refusing,
 *  which cannot happen inside `0..count` — checked anyway, because a silent `""` in
 *  the map would resolve a caller's id to a capability it never named. */
export function readRegistry(
  exports: RawExports,
  countExport: "gm_layout_count" | "gm_post_count" | "gm_analysis_count",
  idExport: "gm_layout_id" | "gm_post_id" | "gm_analysis_id",
): Map<string, number> {
  const ids = new Map<string, number>();
  const count = invoke(countExport, () => exports[countExport]());
  for (let i = 0; i < count; i += 1) {
    const ptr = invoke(idExport, () => exports[idExport](toU32(i)));
    if (ptr === 0) {
      throw new RunRefusedError(`${idExport}(${i}) refused (${codeName(lastError(exports))})`);
    }
    ids.set(decoder.decode(frame(exports, ptr)), i);
  }
  return ids;
}

export function snapshotPtr(
  exports: RawExports,
  exportName: "gm_snapshot_json" | "gm_snapshot_bytes",
  handle: Handle,
): number {
  const ptr = invoke(exportName, () => exports[exportName](toU32(handle)));
  if (ptr !== 0) return ptr;
  const code = lastError(exports);
  if (code === TAMPERED_GEOMETRY_CODE) {
    throw new TamperedGeometryError("a column view wrote a non-finite value since the last run", code);
  }
  throw new RunRefusedError(`${exportName} refused (${codeName(code)})`, code);
}

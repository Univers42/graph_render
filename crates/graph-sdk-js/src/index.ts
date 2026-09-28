// The published entry point (`docs/contract/wasm-abi.md` "SDK surface";
// `harness/sdk-smoke.mjs` imports only this file, never `wasm.ts`/`views.ts` directly).
// `createMotor` loads the module once; `Motor#build`/`#layout`/`#release` are the ABI's
// `gm_build`/`gm_run`/`gm_release`, with u32 coercion (C9), a typed-error policy (every
// refusal is a `GraphMotorError` subclass, never a bare string or a raw
// `WebAssembly.RuntimeError`), and D9's tamper re-check surfaced as `TamperedGeometryError`
// rather than a silent `0`.

import { loadMotor, toU32, type RawExports, type WasmSource } from "./wasm.ts";
import { ColumnViews } from "./views.ts";
import {
  BuildRefusedError,
  InvalidHandleError,
  InvalidOptionsError,
  MotorTrapError,
  RunRefusedError,
  TamperedGeometryError,
  WasmUnavailableError,
  codeName,
} from "./errors.ts";
import {
  ColumnId,
  type Column,
  type EdgeGeometryKind,
  type Handle,
  type MotorOptions,
  type NodeGeometryKind,
  type RunResult,
} from "./types.ts";

export type { WasmSource } from "./wasm.ts";
export { resetForTests } from "./wasm.ts";
export * from "./errors.ts";
export * from "./types.ts";

const NODE_KIND_BY_TAG: readonly NodeGeometryKind[] = ["Point", "Circle", "Box"];
const EDGE_KIND_BY_TAG: readonly EdgeGeometryKind[] = ["Line", "Polyline", "Curve"];
const TAMPERED_GEOMETRY_CODE = 9; // Code::TamperedGeometry, crates/graph-wasm/src/errors.rs
const INVALID_HANDLE_CODE = 1; // Code::InvalidHandle

const encoder = new TextEncoder();
const decoder = new TextDecoder("utf-8", { fatal: true });

/** `MotorOptions`'s closed shape this phase (C16): an unknown key, or an `exec` value
 * other than `"auto"`, is refused loudly rather than silently ignored. */
function checkOptions(options: MotorOptions | undefined): void {
  if (options === undefined) return;
  for (const key of Object.keys(options)) {
    if (key !== "exec") throw new InvalidOptionsError(`unknown option "${key}"`);
  }
  if (options.exec !== undefined && options.exec !== "auto") {
    throw new InvalidOptionsError(
      `options.exec must be "auto" this phase, got ${JSON.stringify(options.exec)} (docs/decisions/compute-tiers.md)`,
    );
  }
}

/** One loaded wasm module and every graph built against it. `createMotor` is the only way
 * to get one — the constructor is private so a `Motor` is never in play without having
 * gone through the loader's kill switch / `initFailed` checks. */
export class Motor {
  readonly #exports: RawExports | null;
  readonly #views: ColumnViews | null;
  readonly #loadError: WasmUnavailableError | null;
  readonly #kinds = new Map<number, { nodeKind: NodeGeometryKind; edgeKind: EdgeGeometryKind }>();
  #layoutIds: Map<string, number> | null = null;

  private constructor(exports: RawExports | null, loadError: WasmUnavailableError | null) {
    this.#exports = exports;
    this.#views = exports === null ? null : new ColumnViews(exports);
    this.#loadError = loadError;
  }

  /** @internal — use {@link createMotor}. Never throws (`prompt.md` §3.2,
   * `phase-04-wasm-sdk.md` step 5: "warn-and-degrade rather than throw on init failure"):
   * a load failure — the kill switch, the latched `initFailed`, or the compile/instantiate
   * itself throwing — resolves to a *degraded* `Motor` instead. Every other method on a
   * degraded motor fails predictably via {@link Motor.#requireLoaded}, so the one place
   * that would otherwise take the host page down with it never does; nothing is fabricated
   * (`docs/contract/wasm-abi.md` "Deviations" explains why a fake handle would be worse). */
  static async create(source: WasmSource, options?: MotorOptions): Promise<Motor> {
    checkOptions(options);
    try {
      const exports = await loadMotor(source);
      return new Motor(exports, null);
    } catch (error) {
      const failure = error instanceof WasmUnavailableError ? error : new WasmUnavailableError("wasm module failed to load", error);
      return new Motor(null, failure);
    }
  }

  /** The current view-invalidation epoch (`views.ts`): moves forward on every call below
   * that can invalidate a previously-returned column view. Read-only; exposed for
   * `harness/wasm-run.mjs --assert-zero-copy` and for a caller that wants to detect "did
   * anything change" without diffing bytes itself. `0` on a degraded motor: nothing has
   * ever run, so nothing has ever moved. */
  get epoch(): number {
    return this.#views === null ? 0 : this.#views.epoch;
  }

  /** Whether this motor actually loaded a module. A degraded motor (see {@link create})
   * answers `false` here without throwing, so a caller can check availability before
   * calling anything that would refuse (`docs/contract/wasm-abi.md` "Loader pattern"). */
  get available(): boolean {
    return this.#exports !== null;
  }

  /** Every method below that needs the real module calls this first: on a degraded motor
   * (`#exports` is `null`) it throws the latched load failure predictably, at first use —
   * never at {@link create} itself, and never a fabricated handle or value. */
  #requireLoaded(): { exports: RawExports; views: ColumnViews } {
    if (this.#exports === null || this.#views === null) {
      throw this.#loadError ?? new WasmUnavailableError("wasm module is not loaded");
    }
    return { exports: this.#exports, views: this.#views };
  }

  #invoke<T>(name: string, fn: () => T): T {
    try {
      return fn();
    } catch (error) {
      if (error instanceof WebAssembly.RuntimeError) throw new MotorTrapError(name, error);
      throw error;
    }
  }

  #lastError(exports: RawExports): number {
    return this.#invoke("gm_last_error", () => exports.gm_last_error());
  }

  /** `[len: u32 LE][len bytes]` at `ptr`, copied out (`docs/contract/wasm-abi.md`
   * "Framed buffers"): this one buffer is reused by the next motor call, so nothing may
   * hold a reference into it past this method returning. */
  #frame(exports: RawExports, ptr: number): Uint8Array {
    const header = new DataView(exports.memory.buffer);
    const len = header.getUint32(ptr, true);
    return new Uint8Array(exports.memory.buffer, ptr + 4, len).slice();
  }

  #layoutIndex(exports: RawExports, layoutId: string): number {
    if (this.#layoutIds === null) {
      const ids = new Map<string, number>();
      const count = this.#invoke("gm_layout_count", () => exports.gm_layout_count());
      for (let i = 0; i < count; i += 1) {
        const ptr = this.#invoke("gm_layout_id", () => exports.gm_layout_id(toU32(i)));
        ids.set(decoder.decode(this.#frame(exports, ptr)), i);
      }
      this.#layoutIds = ids;
    }
    const index = this.#layoutIds.get(layoutId);
    if (index === undefined) throw new RunRefusedError(`unknown layout id "${layoutId}"`);
    return index;
  }

  /** Builds a graph from `ingestJson`, the provisional ingest text
   * (`docs/contract/wasm-abi.md` "Ingest — PROVISIONAL"; Phase 10 owns the real contract).
   * Stages it through `gm_alloc`/`gm_build` and always frees the staging buffer — C7 makes
   * that this method's job, not its caller's, since the caller never sees the pointer. */
  build(ingestJson: string): Handle {
    const { exports, views } = this.#requireLoaded();
    const bytes = encoder.encode(ingestJson);
    const len = toU32(bytes.length);
    const ptr = this.#invoke("gm_alloc", () => exports.gm_alloc(len));
    if (ptr === 0) throw new BuildRefusedError("gm_alloc could not reserve the ingest buffer", this.#lastError(exports));
    try {
      new Uint8Array(exports.memory.buffer, ptr, len).set(bytes);
      const handle = this.#invoke("gm_build", () => exports.gm_build(ptr, len));
      views.bump();
      if (handle === 0) throw new BuildRefusedError("gm_build refused the ingest buffer", this.#lastError(exports));
      return handle as Handle;
    } finally {
      this.#invoke("gm_free", () => exports.gm_free(ptr, len));
      views.bump();
    }
  }

  /** Nodes in `handle`'s topology — available right after {@link build}, before any run.
   * `0` is ambiguous on the wire (a genuinely empty graph, or an invalid handle, C4): this
   * method resolves it via `gm_last_error` so only the real refusal throws. */
  nodeCount(handle: Handle): number {
    const { exports } = this.#requireLoaded();
    const count = this.#invoke("gm_node_count", () => exports.gm_node_count(toU32(handle)));
    if (count !== 0) return count;
    if (this.#lastError(exports) === INVALID_HANDLE_CODE) throw new InvalidHandleError(`handle ${handle} is not live`, INVALID_HANDLE_CODE);
    return count;
  }

  /** Runs the registered layout `layoutId` (e.g. `"layout.grid"`, looked up by scanning
   * `gm_layout_count()`/`gm_layout_id` — never a hard-coded index, C1) over `handle`'s
   * topology at its default parameters (registry layouts take none this phase, C2). */
  layout(handle: Handle, layoutId: string): RunResult {
    const { exports, views } = this.#requireLoaded();
    const index = this.#layoutIndex(exports, layoutId);
    const ok = this.#invoke("gm_run", () => exports.gm_run(toU32(handle), toU32(index), 0, 0));
    views.bump();
    if (ok !== 1) {
      const code = this.#lastError(exports);
      if (code === INVALID_HANDLE_CODE) throw new InvalidHandleError(`handle ${handle} is not live`, code);
      throw new RunRefusedError(`gm_run refused (${codeName(code)})`, code);
    }
    const nodeTag = this.#invoke("gm_geometry_kind", () => exports.gm_geometry_kind(toU32(handle)));
    const edgeTag = this.#invoke("gm_edge_geometry_kind", () => exports.gm_edge_geometry_kind(toU32(handle)));
    const nodeKind = NODE_KIND_BY_TAG[nodeTag];
    const edgeKind = EDGE_KIND_BY_TAG[edgeTag];
    if (nodeKind === undefined || edgeKind === undefined) {
      throw new RunRefusedError(`unknown geometry tag (node ${nodeTag}, edge ${edgeTag})`);
    }
    this.#kinds.set(handle, { nodeKind, edgeKind });
    return { handle, nodeKind, edgeKind, nodeCount: this.nodeCount(handle) };
  }

  /** The column `columnId` of `handle`'s last run, or `null` if it is reserved or does not
   * apply to this run's geometry kind (C3) — call {@link layout} first; a handle with no
   * successful run yet is refused, not read as "every column absent". */
  column(handle: Handle, columnId: ColumnId): Column {
    const { views } = this.#requireLoaded();
    const kinds = this.#kinds.get(handle);
    if (kinds === undefined) throw new InvalidHandleError(`handle ${handle} has no successful run yet`);
    return views.get(toU32(handle) as Handle, columnId, kinds.nodeKind, kinds.edgeKind);
  }

  /** The canonical JSON face of `handle`'s last run. Refuses with
   * {@link TamperedGeometryError} if a column view wrote a non-finite value into the
   * motor's own buffers since that run (D9 re-validation, C8) — this is what makes writing
   * NaN through a zero-copy view an error here, not a value that silently reaches JSON. */
  toJSON(handle: Handle): string {
    const { exports } = this.#requireLoaded();
    return decoder.decode(this.#frame(exports, this.#snapshotPtr(exports, "gm_snapshot_json", handle)));
  }

  /** The binary face of `handle`'s last run. Same D9 re-validation as {@link toJSON}. */
  toBytes(handle: Handle): Uint8Array {
    const { exports } = this.#requireLoaded();
    return this.#frame(exports, this.#snapshotPtr(exports, "gm_snapshot_bytes", handle));
  }

  #snapshotPtr(exports: RawExports, exportName: "gm_snapshot_json" | "gm_snapshot_bytes", handle: Handle): number {
    const ptr = this.#invoke(exportName, () => exports[exportName](toU32(handle)));
    if (ptr !== 0) return ptr;
    const code = this.#lastError(exports);
    if (code === TAMPERED_GEOMETRY_CODE) {
      throw new TamperedGeometryError("a column view wrote a non-finite value since the last run", code);
    }
    throw new RunRefusedError(`${exportName} refused (${codeName(code)})`, code);
  }

  /** Releases `handle`. Its id is never reissued (C6): using it again after this always
   * reads {@link InvalidHandleError}, never a different, later graph. */
  release(handle: Handle): void {
    const { exports, views } = this.#requireLoaded();
    this.#invoke("gm_release", () => exports.gm_release(toU32(handle)));
    views.bump();
    views.forget(handle);
    this.#kinds.delete(handle);
  }
}

/** Loads the wasm motor (once per session — see `wasm.ts`) and returns a {@link Motor}
 * bound to it. `options` this phase accepts only `{}` or `{ exec: "auto" }` (C16); any
 * other shape is refused before the module is even asked to load. */
export async function createMotor(source: WasmSource, options?: MotorOptions): Promise<Motor> {
  return Motor.create(source, options);
}

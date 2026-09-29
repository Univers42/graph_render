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
  AnalysisRefusedError,
  BuildRefusedError,
  InvalidHandleError,
  InvalidOptionsError,
  MotorTrapError,
  PostRefusedError,
  RunRefusedError,
  TamperedGeometryError,
  WasmUnavailableError,
  codeName,
} from "./errors.ts";
import {
  ColumnId,
  type AnalysisResult,
  type AnalysisValueKind,
  type Column,
  type EdgeGeometryKind,
  type Handle,
  type MotorOptions,
  type NodeGeometryKind,
  type PostResult,
  type RunResult,
} from "./types.ts";

export type { WasmSource } from "./wasm.ts";
export { resetForTests } from "./wasm.ts";
export * from "./errors.ts";
export * from "./types.ts";

const NODE_KIND_BY_TAG: readonly NodeGeometryKind[] = ["Point", "Circle", "Box"];
const EDGE_KIND_BY_TAG: readonly EdgeGeometryKind[] = ["Line", "Polyline", "Curve"];

/** A handle's two geometry tags, as this SDK records them: what decides whether a
 *  column is present (C3), and what a run or a post pass reports back. */
interface GeometryKinds {
  nodeKind: NodeGeometryKind;
  edgeKind: EdgeGeometryKind;
}
const TAMPERED_GEOMETRY_CODE = 9; // Code::TamperedGeometry, crates/graph-wasm/src/errors.rs
const INVALID_HANDLE_CODE = 1; // Code::InvalidHandle
const NO_GEOMETRY_CODE = 10; // Code::NoGeometryYet

/** One optional member of an analysis face, when it is present and of the right type.
 *  An absent member is `undefined`; a member of the *wrong* type is a refusal, never a
 *  silent `undefined` — a caller told `converged: undefined` would read it as "no flag
 *  was handed back" and trust numbers that were never verified.
 */
function optionalBoolean(face: Record<string, unknown>, key: string, asked: string): boolean | undefined {
  const value = face[key];
  if (value === undefined) return undefined;
  if (typeof value !== "boolean") throw new AnalysisRefusedError(`${asked}: ${key} is not a boolean`);
  return value;
}

/** As {@link optionalBoolean}, for a number. `max` is a depth level and `modularity` a
 *  score; both are wire numbers, and a non-number in either is a broken face.
 */
function optionalNumber(face: Record<string, unknown>, key: string, asked: string): number | undefined {
  const value = face[key];
  if (value === undefined) return undefined;
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new AnalysisRefusedError(`${asked}: ${key} is not a finite number`);
  }
  return value;
}

/** The ABI's canonical JSON face (`docs/contract/wasm-abi.md` "ANALYSIS"), parsed into
 *  {@link AnalysisResult} and **checked member by member rather than cast**. A face whose
 *  `id`, `kind` or `nodeCount` did not agree with itself would otherwise reach a caller
 *  as a plausible-looking object holding someone else's numbers, which is the one failure
 *  mode a typed wrapper exists to prevent — so each disagreement is an
 *  {@link AnalysisRefusedError} naming what did not match.
 */
export function parseAnalysisFace(text: string, asked: string): AnalysisResult {
  const parsed: unknown = JSON.parse(text);
  if (typeof parsed !== "object" || parsed === null) {
    throw new AnalysisRefusedError(`${asked}: the result is not a JSON object`);
  }
  const face = parsed as Record<string, unknown>;
  const { id, kind, nodeCount, values } = face;
  if (id !== asked) {
    throw new AnalysisRefusedError(`${asked}: the motor answered for "${String(id)}"`);
  }
  if (kind !== "f64" && kind !== "u32") {
    throw new AnalysisRefusedError(`${asked}: unknown value kind ${JSON.stringify(kind)}`);
  }
  if (!Array.isArray(values) || values.some((v) => typeof v !== "number")) {
    throw new AnalysisRefusedError(`${asked}: values is not an array of numbers`);
  }
  const numbers = values as number[];
  if (nodeCount !== numbers.length) {
    throw new AnalysisRefusedError(`${asked}: nodeCount ${String(nodeCount)} but ${numbers.length} values`);
  }
  const typedKind: AnalysisValueKind = kind;
  return {
    id,
    kind: typedKind,
    nodeCount: numbers.length,
    values: numbers,
    converged: optionalBoolean(face, "converged", asked),
    modularity: optionalNumber(face, "modularity", asked),
    max: optionalNumber(face, "max", asked),
  };
}

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
  readonly #kinds = new Map<Handle, GeometryKinds>();
  #layoutIds: Map<string, number> | null = null;
  #postIds: Map<string, number> | null = null;
  #analysisIds: Map<string, number> | null = null;

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

  /** Every registered layout id, in registry order — this SDK's view of the module's own
   * registry (`gm_layout_count`/`gm_layout_id`, C1). A consumer asks what the module can
   * run instead of hard-coding a name, so a layout registered after this SDK was written
   * is discoverable with no SDK change; `layout` below resolves names through the same
   * map, so the registry is scanned once per motor, never once per call. Refuses on a
   * degraded motor like every other method that needs the module, rather than reporting
   * an empty registry that would read as "this module has no layouts". */
  layouts(): readonly string[] {
    this.#requireLoaded();
    return [...this.#registry().keys()];
  }

  /** The layout registry's id -> index map, read once per motor (C1). */
  #registry(): Map<string, number> {
    if (this.#layoutIds === null) {
      this.#layoutIds = this.#readRegistry("gm_layout_count", "gm_layout_id");
    }
    return this.#layoutIds;
  }

  #layoutIndex(layoutId: string): number {
    const index = this.#registry().get(layoutId);
    if (index === undefined) throw new RunRefusedError(`unknown layout id "${layoutId}"`);
    return index;
  }

  /** Scans one registry the way every registry in this ABI is discovered: the count,
   *  then a framed id per index. Written once so the three cannot drift into three
   *  differently-shaped scans (C1). A `0` from the id export is the motor refusing,
   *  which cannot happen inside `0..count` — checked anyway, because a silent `""` in
   *  the map would resolve a caller's id to a capability it never named. */
  #readRegistry(
    countExport: "gm_layout_count" | "gm_post_count" | "gm_analysis_count",
    idExport: "gm_layout_id" | "gm_post_id" | "gm_analysis_id",
  ): Map<string, number> {
    const { exports } = this.#requireLoaded();
    const ids = new Map<string, number>();
    const count = this.#invoke(countExport, () => exports[countExport]());
    for (let i = 0; i < count; i += 1) {
      const ptr = this.#invoke(idExport, () => exports[idExport](toU32(i)));
      if (ptr === 0) {
        throw new RunRefusedError(`${idExport}(${i}) refused (${codeName(this.#lastError(exports))})`);
      }
      ids.set(decoder.decode(this.#frame(exports, ptr)), i);
    }
    return ids;
  }

  /** Every registered POST capability id, in registry order — this SDK's view of the
   *  module's own registry (`gm_post_count`/`gm_post_id`, C1). Discoverable for the same
   *  reason {@link Motor.layouts} is: a capability registered after this SDK was written
   *  must be usable with no SDK change. Refuses on a degraded motor rather than
   *  answering `[]`, which would read as "this module has no post capabilities". */
  posts(): readonly string[] {
    this.#requireLoaded();
    return [...this.#postRegistry().keys()];
  }

  #postRegistry(): Map<string, number> {
    if (this.#postIds === null) {
      this.#postIds = this.#readRegistry("gm_post_count", "gm_post_id");
    }
    return this.#postIds;
  }

  #postIndex(postId: string): number {
    const index = this.#postRegistry().get(postId);
    if (index === undefined) throw new PostRefusedError(`unknown post id "${postId}"`);
    return index;
  }

  /** Every registered analysis id, in registry order (`gm_analysis_count`/
   *  `gm_analysis_id`, C1) — the discoverability {@link Motor.layouts} and
   *  {@link Motor.posts} both exist for, and refused rather than empty on a degraded
   *  motor. */
  analyses(): readonly string[] {
    this.#requireLoaded();
    return [...this.#analysisRegistry().keys()];
  }

  #analysisRegistry(): Map<string, number> {
    if (this.#analysisIds === null) {
      this.#analysisIds = this.#readRegistry("gm_analysis_count", "gm_analysis_id");
    }
    return this.#analysisIds;
  }

  #analysisIndex(analysisId: string): number {
    const index = this.#analysisRegistry().get(analysisId);
    if (index === undefined) throw new AnalysisRefusedError(`unknown analysis id "${analysisId}"`);
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

  /** Runs the registered layout `layoutId` (e.g. `"layout.grid"`, from
   * {@link Motor.layouts} — the id is resolved through `gm_layout_count`/`gm_layout_id`,
   * never a hard-coded index, C1) over `handle`'s topology at its default parameters
   * (registry layouts take none this phase, C2). */
  layout(handle: Handle, layoutId: string): RunResult {
    const { exports, views } = this.#requireLoaded();
    const index = this.#layoutIndex(layoutId);
    const ok = this.#invoke("gm_run", () => exports.gm_run(toU32(handle), toU32(index), 0, 0));
    views.bump();
    if (ok !== 1) {
      const code = this.#lastError(exports);
      if (code === INVALID_HANDLE_CODE) throw new InvalidHandleError(`handle ${handle} is not live`, code);
      throw new RunRefusedError(`gm_run refused (${codeName(code)})`, code);
    }
    const { nodeKind, edgeKind } = this.#kindsOf(exports, handle, layoutId);
    return { handle, nodeKind, edgeKind, nodeCount: this.nodeCount(handle) };
  }

  /** The two geometry tags, read back and recorded so {@link Motor.column} can decide
   *  presence from them (C3). Shared by `layout` and `post` because a POST pass
   *  changes the *edge* kind — routing over a `Line` layout yields a `Polyline`, and a
   *  style declares its own — so the cached kinds have to be re-read after a pass or
   *  every column read would be decided against the pre-pass kind. */
  #kindsOf(exports: RawExports, handle: Handle, what: string): GeometryKinds {
    const nodeTag = this.#invoke("gm_geometry_kind", () => exports.gm_geometry_kind(toU32(handle)));
    const edgeTag = this.#invoke("gm_edge_geometry_kind", () => exports.gm_edge_geometry_kind(toU32(handle)));
    const nodeKind = NODE_KIND_BY_TAG[nodeTag];
    const edgeKind = EDGE_KIND_BY_TAG[edgeTag];
    if (nodeKind === undefined || edgeKind === undefined) {
      throw new RunRefusedError(`${what}: unknown geometry tag (node ${nodeTag}, edge ${edgeTag})`);
    }
    const kinds = { nodeKind, edgeKind };
    this.#kinds.set(handle, kinds);
    return kinds;
  }

  /** Runs the registered POST capability `postId` (from {@link Motor.posts}; the id is
   *  resolved through `gm_post_count`/`gm_post_id`, never a hard-coded index, C1) over
   *  `handle`'s last successful layout run, **replacing that run's edge geometry**. The
   *  handle's columns then read the new edges, so a caller reads `EdgeOffsets`/`EdgePts`
   *  exactly as it would after {@link Motor.layout} — the pass is invisible to the
   *  transport, which is the point: POST is a stage, not a second transport.
   *
   *  A handle with no successful layout run is refused (`NoGeometryYet`): there is
   *  nothing for a pass to draw over. A refused pass leaves the handle's geometry exactly
   *  as it was, so a failed call never serves a half-applied drawing.
   *
   *  Each pass reads the **layout's** edges, never the previous pass's, so running style
   *  then bundle gives the same answer as running bundle once.
   */
  post(handle: Handle, postId: string): PostResult {
    const { exports, views } = this.#requireLoaded();
    const index = this.#postIndex(postId);
    const ok = this.#invoke("gm_post_run", () => exports.gm_post_run(toU32(handle), toU32(index)));
    views.bump();
    if (ok !== 1) {
      const code = this.#lastError(exports);
      if (code === INVALID_HANDLE_CODE) throw new InvalidHandleError(`handle ${handle} is not live`, code);
      if (code === NO_GEOMETRY_CODE) throw new PostRefusedError(`handle ${handle} has no successful layout run to draw over`, code);
      throw new PostRefusedError(`gm_post_run refused (${codeName(code)})`, code);
    }
    const { nodeKind, edgeKind } = this.#kindsOf(exports, handle, postId);
    return { handle, id: postId, nodeKind, edgeKind, nodeCount: this.nodeCount(handle) };
  }

  /** Runs the registered analysis `analysisId` (from {@link Motor.analyses}) over
   *  `handle`'s topology and returns its typed result. **No layout run is required** —
   *  every analysis is a function of the topology, so this works straight after
   *  {@link Motor.build}; only an unknown handle or an unregistered id throws. */
  analysis(handle: Handle, analysisId: string): AnalysisResult {
    const { exports } = this.#requireLoaded();
    const index = this.#analysisIndex(analysisId);
    const ptr = this.#invoke("gm_analysis_run", () => exports.gm_analysis_run(toU32(handle), toU32(index)));
    if (ptr === 0) {
      const code = this.#lastError(exports);
      if (code === INVALID_HANDLE_CODE) throw new InvalidHandleError(`handle ${handle} is not live`, code);
      throw new AnalysisRefusedError(`gm_analysis_run refused (${codeName(code)})`, code);
    }
    return parseAnalysisFace(decoder.decode(this.#frame(exports, ptr)), analysisId);
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

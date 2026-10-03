// The published entry point (`docs/contract/wasm-abi.md` "SDK surface";
// `harness/sdk-smoke.mjs` imports only this file, never `wasm.ts`/`views.ts` directly).
// `createMotor` loads the module once; `Motor#build`/`#run`/`#layout`/`#release` are the
// ABI's `gm_build`/`gm_run`/`gm_release`, with u32 coercion (C9), a typed-error policy
// (every refusal is a `GraphMotorError` subclass, never a bare string or a raw
// `WebAssembly.RuntimeError`), and D9's tamper re-check surfaced as `TamperedGeometryError`
// rather than a silent `0`.
//
// **Two ways in, deliberately.** `build` is the ABI's `gm_build`: the provisional
// node/edge JSON. `buildContract` is `gm_build_contract`: the phase-10 ingest contract
// document, which is what this package's own adapters produce and what the motor derives a
// graph from. Each refuses the other's document with its own error class, because they are
// different documents with different meanings — `docs/contract/wasm-abi.md` "Two build
// paths" has the table and the reasoning.
//
// **What is not here is what has a buffer or a rule behind it.** The three thin methods
// `build`, `run` and `layout` delegate to `staging.ts` and `params.ts`, which own the two
// documents' contracts and the parameter buffer, and `views.ts` owns the column table,
// the zero-copy cache and the geometry kinds a run produced. The surface below is this
// class's typed face; the reasoning sits with the code that enforces it.

import { loadMotor, toU32, type RawExports, type WasmSource } from "./wasm.ts";
import { ColumnViews } from "./views.ts";
import { ForceSession } from "./force.ts";
import { InvalidHandleError, WasmUnavailableError, codeName } from "./errors.ts";
import { ColumnId, type AnalysisResult, type Column, type ForceEngine, type ForceParams, type Handle } from "./types.ts";
import type { MotorOptions, PostResult, RunOptions, RunResult } from "./types.ts";
import { INVALID_HANDLE_CODE, invoke, lastError, type Loaded } from "./calls.ts";
import { checkOptions } from "./options.ts";
import { Registries } from "./registries.ts";
import { LayoutParams, runAtParams, type LayoutParamSpec } from "./params.ts";
import { CONTRACT_BUILD, INGEST_BUILD, buildStaged } from "./staging.ts";
import { analysisRun, postRun } from "./stages.ts";

export { resetForTests, type WasmSource } from "./wasm.ts";
export * from "./errors.ts";
export * from "./types.ts";
export { ForceSession, PARAMS_BYTES, encodeParams, decodeParams } from "./force.ts";

export { parseAnalysisFace } from "./analysis-face.ts";
export * from "./adapters.ts";
export * from "./params.ts";

/** One loaded wasm module and every graph built against it. `createMotor` is the only way
 * to get one — the constructor is private so a `Motor` is never in play without having
 * gone through the loader's kill switch / `initFailed` checks. */
export class Motor {
  readonly #exports: RawExports | null;
  readonly #views: ColumnViews | null;
  readonly #loadError: WasmUnavailableError | null;
  readonly #registries = new Registries();
  /** This motor's published-parameter schemas, read once each: a schema cannot change
   *  under a live module (`docs/decisions/layout-params.md`). */
  readonly #params = new LayoutParams();

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
  #requireLoaded(): Loaded {
    if (this.#exports === null || this.#views === null) {
      throw this.#loadError ?? new WasmUnavailableError("wasm module is not loaded");
    }
    return { exports: this.#exports, views: this.#views };
  }

  /** Every registered layout id, in registry order — this SDK's view of the module's own
   * registry (`gm_layout_count`/`gm_layout_id`, C1). A consumer asks what the module can
   * run instead of hard-coding a name, so a layout registered after this SDK was written
   * is discoverable with no SDK change; `layout` below resolves names through the same
   * map, so the registry is scanned once per motor, never once per call. Refuses on a
   * degraded motor like every other method that needs the module, rather than reporting
   * an empty registry that would read as "this module has no layouts". */
  layouts(): readonly string[] {
    const { exports } = this.#requireLoaded();
    return [...this.#registries.layouts(exports).keys()];
  }

  /** Every registered POST capability id, in registry order (`gm_post_count`/
   *  `gm_post_id`, C1). Discoverable for the reason {@link Motor.layouts} gives, and
   *  refused on a degraded motor for the same one: `[]` would read as "this module has
   *  no post capabilities". */
  posts(): readonly string[] {
    const { exports } = this.#requireLoaded();
    return [...this.#registries.posts(exports).keys()];
  }

  /** Every registered analysis id, in registry order (`gm_analysis_count`/
   *  `gm_analysis_id`, C1), for the discoverability {@link Motor.layouts} exists for. */
  analyses(): readonly string[] {
    const { exports } = this.#requireLoaded();
    return [...this.#registries.analyses(exports).keys()];
  }

  /** Builds a graph from `ingestJson`, the provisional node/edge JSON: the ABI's
   *  `gm_build`, the document the host studio and the hash gate already speak. Staged and
   *  freed here (C7); `staging.ts` carries what the buffer accepts and refuses, and
   *  {@link buildContract} is the other way in. */
  build(ingestJson: string): Handle {
    return buildStaged(this.#requireLoaded(), ingestJson, INGEST_BUILD);
  }

  /** Builds a graph from `contractJson`, an **ingest contract** document — the one shape
   *  every source maps to. A provisional node/edge document is refused by it, because it is
   *  not a contract. Staged and freed exactly as {@link build} is (C7); `staging.ts` carries
   *  the buffer's contract. */
  buildContract(contractJson: string): Handle {
    return buildStaged(this.#requireLoaded(), contractJson, CONTRACT_BUILD);
  }

  /** Nodes in `handle`'s topology — available right after {@link build}, before any run.
   * `0` is ambiguous on the wire (a genuinely empty graph, or an invalid handle, C4): this
   * method resolves it via `gm_last_error` so only the real refusal throws. */
  nodeCount(handle: Handle): number {
    const { exports } = this.#requireLoaded();
    const count = invoke("gm_node_count", () => exports.gm_node_count(toU32(handle)));
    if (count !== 0) return count;
    if (lastError(exports) === INVALID_HANDLE_CODE) throw new InvalidHandleError(`handle ${handle} is not live`, INVALID_HANDLE_CODE);
    return count;
  }

  /** Every parameter the registered layout `layoutId` publishes, in the order a run's
   *  parameter buffer carries them — `gm_layout_params` over the same index
   *  {@link Motor.layout} resolves the id to (`docs/decisions/layout-params.md`). Read
   *  once per motor and cached by the `LayoutParams` this holds; the decode and the
   *  refusal live in `params.ts`. */
  layoutParams(layoutId: string): LayoutParamSpec[] {
    const { exports } = this.#requireLoaded();
    return this.#params.read(exports, layoutId, this.#registries.layoutIndex(exports, layoutId));
  }

  /** Runs the registered layout `layoutId` (e.g. `"layout.grid"`, from
   *  {@link Motor.layouts} — the id is resolved through `gm_layout_count`/`gm_layout_id`,
   *  never a hard-coded index, C1) over `handle`'s topology, at `options.params` where it
   *  is given and at the layout's own defaults where it is not.
   *
   *  `options.params` is keyed by the names {@link Motor.layoutParams} publishes; a name
   *  it does not publish is a `RangeError` and a published name left out takes its
   *  default. Values are sent as written — the motor, not this SDK, decides whether one
   *  is in range, and refuses it with `ParamOutOfRange` rather than clamping it.
   *
   *  {@link Motor.layout} is this method with no options and is kept so every existing
   *  caller is unchanged. */
  run(handle: Handle, layoutId: string, options?: RunOptions): RunResult {
    const { exports, views } = this.#requireLoaded();
    const layoutIndex = this.#registries.layoutIndex(exports, layoutId);
    const values = options?.params;
    const specs = values === undefined ? [] : this.#params.read(exports, layoutId, layoutIndex);
    runAtParams(exports, views, { handle, layoutIndex, specs, values });
    const { nodeKind, edgeKind, dim } = views.record(handle, layoutId);
    return { handle, nodeKind, edgeKind, nodeCount: this.nodeCount(handle), dim };
  }

  /** Runs the registered layout `layoutId` over `handle`'s topology at its default
   * parameters. The two-argument form of {@link Motor.run}, kept so no caller written
   * against the pre-ABI-2 surface changes. */
  layout(handle: Handle, layoutId: string): RunResult {
    return this.run(handle, layoutId);
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
    const postIndex = this.#registries.postIndex(exports, postId);
    const { nodeKind, edgeKind, dim } = postRun(exports, views, { handle, postId, postIndex });
    return { handle, id: postId, nodeKind, edgeKind, nodeCount: this.nodeCount(handle), dim };
  }

  /** Runs the registered analysis `analysisId` (from {@link Motor.analyses}) over
   *  `handle`'s topology and returns its typed result. **No layout run is required** —
   *  every analysis is a function of the topology, so this works straight after
   *  {@link Motor.build}; only an unknown handle or an unregistered id throws. */
  analysis(handle: Handle, analysisId: string): AnalysisResult {
    const { exports } = this.#requireLoaded();
    const analysisIndex = this.#registries.analysisIndex(exports, analysisId);
    return analysisRun(exports, { handle, analysisId, analysisIndex });
  }

  /** The column `columnId` of `handle`'s last run, or `null` if it is reserved or does not
   * apply to this run's geometry kind (C3) — call {@link layout} first; a handle with no
   * successful run yet is refused, not read as "every column absent". The presence table
   * and the zero-copy cache are `views.ts`'s, which is also where the kinds this needs
   * come from. */
  column(handle: Handle, columnId: ColumnId): Column {
    return this.#requireLoaded().views.get(toU32(handle) as Handle, columnId);
  }

  /** The canonical JSON face of `handle`'s last run. Refuses with
   * {@link TamperedGeometryError} if a column view wrote a non-finite value into the
   * motor's own buffers since that run (D9 re-validation, C8) — this is what makes writing
   * NaN through a zero-copy view an error here, not a value that silently reaches JSON. */
  toJSON(handle: Handle): string {
    return this.#requireLoaded().views.toJSON(handle);
  }

  /** The binary face of `handle`'s last run. Same D9 re-validation as {@link toJSON}. */
  toBytes(handle: Handle): Uint8Array {
    return this.#requireLoaded().views.toBytes(handle);
  }

  /** Releases `handle`. Its id is never reissued (C6): using it again after this always
   * reads {@link InvalidHandleError}, never a different, later graph. */
  release(handle: Handle): void {
    const { exports, views } = this.#requireLoaded();
    invoke("gm_release", () => exports.gm_release(toU32(handle)));
    views.bump();
    views.forget(handle);
  }

  /** Starts a **live force session** over `handle`'s topology
   * (`docs/decisions/force-wasm-abi.md`), the one surface that runs a layout tick by tick
   * instead of to a finished picture.
   *
   *  `params` is optional and partial: an omitted field keeps the motor's own value for it, so
   *  a host can move one knob without knowing the other twelve and without a copy of the
   *  defaults in its own source going stale. Every field is range-checked by the motor and
   *  **never clamped**, so an out-of-range value is a {@link ForceSessionRefusedError} with the
   *  session left exactly as it was — and a refused creation leaves no session behind.
   *
   *  **No layout run is required**, exactly as for {@link Motor.analysis}: the session is built
   *  from the topology and seeded on the engine's own spiral, so this works straight after
   *  {@link Motor.build}. The session does not read the graph handle's snapshot, does not
   *  replace it, and **outlives it** — {@link Motor.release} on `handle` leaves the session
   *  running, and the session is released with its own {@link ForceSession.release}.
   *
   *  The two have separate id spaces and separate error codes (`InvalidHandle` against
   *  `InvalidSession`), so a caller debugging a dead one is never sent looking at the other.
   *
   *  `engine` picks the tick ({@link ForceEngine}); every other method is the same for both. */
  forceSession(handle: Handle, params?: Partial<ForceParams>, engine?: ForceEngine): ForceSession {
    return new ForceSession(this.#requireLoaded(), handle, params, engine);
  }
}

/** Loads the wasm motor (once per session — see `wasm.ts`) and returns a {@link Motor}
 * bound to it. `options` this phase accepts only `{}` or `{ exec: "auto" }` (C16); any
 * other shape is refused before the module is even asked to load. */
export async function createMotor(source: WasmSource, options?: MotorOptions): Promise<Motor> {
  return Motor.create(source, options);
}

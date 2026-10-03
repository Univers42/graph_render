// The published entry point (`docs/contract/wasm-abi.md` "SDK surface";
// `harness/sdk-smoke.mjs` imports only this file, never `wasm.ts`/`views.ts` directly).
// `createMotor` loads the module once; `Motor#build`/`#layout`/`#release` are the ABI's
// `gm_build`/`gm_run`/`gm_release`, with u32 coercion (C9), a typed-error policy (every
// refusal is a `GraphMotorError` subclass, never a bare string or a raw
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
// The class lives here and `index.ts` is the barrel that publishes it, because the class is
// the implementation and the barrel is the promise: a consumer imports one file name and gets
// the whole surface, and the file that has to stay small (the barrel's export list) is not
// the one that grew every method. The three stages this class delegates to are `stages.ts`.

import { loadMotor, toU32, type WasmSource } from "./wasm.ts";
import { loadThreaded } from "./threads.ts";
import { ColumnViews, isRegisteredColumn } from "./views.ts";
import { ForceSession } from "./force.ts";
import { AbiContractError, InvalidHandleError, WasmUnavailableError } from "./errors.ts";
import { INVALID_HANDLE_CODE, invoke, lastError } from "./calls.ts";
import { ColumnId, type AnalysisResult, type Column, type ForceEngine, type ForceParams, type Handle } from "./types.ts";
import type { MotorOptions, PostResult, RunOptions, RunResult } from "./types.ts";
import { checkOptions } from "./options.ts";
import type { GeometryKinds } from "./geometry-kinds.ts";
import { Registries } from "./registries.ts";
import { CONTRACT_BUILD, INGEST_BUILD, buildStaged } from "./staging.ts";
import { LayoutParams, type LayoutParamSpec } from "./params.ts";
import { nodeCount, runAnalysis, runLayout, runPost, snapshotBytes, snapshotText, type StageContext } from "./stages.ts";

/** One loaded wasm module and every graph built against it. `createMotor` is the only way
 * to get one — the constructor is private so a `Motor` is never in play without having
 * gone through the loader's kill switch / `initFailed` checks. */
export class Motor {
  readonly #context: StageContext | null;
  readonly #loadError: WasmUnavailableError | null;
  /** Handles this SDK has released (C6: an id is never reissued). Kept so a released handle
   *  is refused as *released* — with the recorded `InvalidHandle` code — rather than as
   *  "never laid out", which is what a caller's `#kinds` lookup alone could tell it. */
  readonly #released = new Set<Handle>();

  private constructor(context: StageContext | null, loadError: WasmUnavailableError | null) {
    this.#context = context;
    this.#loadError = loadError;
  }

  /** @internal — use {@link createMotor}. Never throws (`prompt.md` §3.2,
   *  `phase-04-wasm-sdk.md` step 5: "warn-and-degrade rather than throw on init failure"):
   *  a load failure — the kill switch, the latched `initFailed`, or the compile/instantiate
   *  itself throwing — resolves to a *degraded* `Motor` instead. Every other method on a
   *  degraded motor fails predictably via {@link Motor.#requireLoaded}, so the one place
   *  that would otherwise take the host page down with it never does; nothing is fabricated
   *  (`docs/contract/wasm-abi.md` "Deviations" explains why a fake handle would be worse). */
  static async create(source: WasmSource, options?: MotorOptions): Promise<Motor> {
    checkOptions(options);
    try {
      const exports = await (options?.threads === undefined ? loadMotor(source) : loadThreaded(source, options.threads));
      const context: StageContext = {
        exports,
        views: new ColumnViews(exports),
        registries: new Registries(),
        kinds: new Map<Handle, GeometryKinds>(),
        params: new LayoutParams(),
      };
      return new Motor(context, null);
    } catch (error) {
      const failure = error instanceof WasmUnavailableError ? error : new WasmUnavailableError("wasm module failed to load", error);
      return new Motor(null, failure);
    }
  }

  /** The current view-invalidation epoch (`views.ts`): moves forward on every call below
   *  that can invalidate a previously-returned column view. Read-only; exposed for
   *  `harness/wasm-run.mjs --assert-zero-copy` and for a caller that wants to detect "did
   *  anything change" without diffing bytes itself. `0` on a degraded motor: nothing has
   *  ever run, so nothing has ever moved. */
  get epoch(): number {
    return this.#context?.views.epoch ?? 0;
  }

  /** Whether this motor actually loaded a module. A degraded motor (see {@link create})
   *  answers `false` here without throwing, so a caller can check availability before
   *  calling anything that would refuse (`docs/contract/wasm-abi.md` "Loader pattern"). */
  get available(): boolean {
    return this.#context !== null;
  }

  /** Every method below that needs the real module calls this first: on a degraded motor
   *  (`#context` is `null`) it throws the latched load failure predictably, at first use —
   *  never at {@link create} itself, and never a fabricated handle or value. */
  #requireLoaded(): StageContext {
    if (this.#context === null) {
      throw this.#loadError ?? new WasmUnavailableError("wasm module is not loaded");
    }
    return this.#context;
  }

  /** Every registered layout id, in registry order — this SDK's view of the module's own
   *  registry (`gm_layout_count`/`gm_layout_id`, C1). A consumer asks what the module can
   *  run instead of hard-coding a name, so a layout registered after this SDK was written
   *  is discoverable with no SDK change; {@link Motor.layout} resolves names through the same
   *  map, so the registry is scanned once per motor, never once per call. Refuses on a
   *  degraded motor rather than reporting an empty registry that would read as "this module
   *  has no layouts". */
  layouts(): readonly string[] {
    const { exports, registries } = this.#requireLoaded();
    return registries.layouts(exports);
  }

  /** Every registered POST capability id, in registry order — this SDK's view of the
   *  module's own registry (`gm_post_count`/`gm_post_id`, C1). Discoverable for the same
   *  reason {@link Motor.layouts} is, and refused rather than empty on a degraded motor. */
  posts(): readonly string[] {
    const { exports, registries } = this.#requireLoaded();
    return registries.posts(exports);
  }

  /** Every registered analysis id, in registry order (`gm_analysis_count`/`gm_analysis_id`,
   *  C1), for the same reason and refused the same way. */
  analyses(): readonly string[] {
    const { exports, registries } = this.#requireLoaded();
    return registries.analyses(exports);
  }

  /** Builds a graph from `ingestJson`, the provisional node/edge JSON: the ABI's `gm_build`,
   *  the document the host studio and the hash gate already speak. Staged and freed here
   *  (C7); `staging.ts` carries what the buffer accepts and refuses, and
   *  {@link Motor.buildContract} is the other way in. */
  build(ingestJson: string): Handle {
    return buildStaged(this.#requireLoaded(), ingestJson, INGEST_BUILD);
  }

  /** Builds a graph from `contractJson`, an **ingest contract** document — the one shape every
   *  source maps to (`docs/contract/ingest-schema.json`, written by this package's own
   *  adapters). A provisional node/edge document is refused with `ContractRefusedError`,
   *  because it is not a contract. Staged and freed exactly as {@link Motor.build} is (C7);
   *  `staging.ts` carries why the two ways in stay separate. */
  buildContract(contractJson: string): Handle {
    return buildStaged(this.#requireLoaded(), contractJson, CONTRACT_BUILD);
  }

  /** Nodes in `handle`'s topology — available right after {@link Motor.build}, before any run.
   *  `0` is ambiguous on the wire (a genuinely empty graph, or an invalid handle, C4): this
   *  method resolves it via `gm_last_error` so only the real refusal throws. */
  nodeCount(handle: Handle): number {
    const { exports } = this.#requireLoaded();
    return nodeCount(exports, handle);
  }

  /** Every parameter the registered layout `layoutId` publishes, in the order a run's
   *  parameter buffer carries them — `gm_layout_params` over the index {@link Motor.run}
   *  resolves the id to, read once per motor (`docs/decisions/layout-params.md`). */
  layoutParams(layoutId: string): LayoutParamSpec[] {
    const { exports, registries, params } = this.#requireLoaded();
    return params.read(exports, layoutId, registries.layoutIndex(exports, layoutId));
  }

  /** Runs the registered layout `layoutId` (from {@link Motor.layouts}; resolved through
   *  `gm_layout_count`/`gm_layout_id`, never a hard-coded index, C1) over `handle`'s topology,
   *  at `options.params` where given and at the layout's own defaults where not. A name
   *  {@link Motor.layoutParams} does not publish is a `RangeError`; a value is sent as written
   *  and the motor, not this SDK, refuses one out of range (`ParamOutOfRange`, never clamped). */
  run(handle: Handle, layoutId: string, options?: RunOptions): RunResult {
    const ctx = this.#requireLoaded();
    const run = runLayout(ctx, handle, layoutId, options?.params);
    return { handle, nodeKind: run.nodeKind, edgeKind: run.edgeKind, nodeCount: nodeCount(ctx.exports, handle), dim: run.dim };
  }

  /** {@link Motor.run} at the layout's defaults, kept so every pre-ABI-2 caller is unchanged. */
  layout(handle: Handle, layoutId: string): RunResult {
    return this.run(handle, layoutId);
  }

  /** Runs the registered POST capability `postId` (from {@link Motor.posts}; the id is
   *  resolved through `gm_post_count`/`gm_post_id`, never a hard-coded index, C1) over
   *  `handle`'s last successful layout run, **replacing that run's edge geometry**. The
   *  handle's columns then read the new edges, so a caller reads `EdgeOffsets`/`EdgePts`
   *  exactly as it would after {@link Motor.layout} — the pass is invisible to the transport,
   *  which is the point: POST is a stage, not a second transport.
   *
   *  Each pass reads the **layout's** edges, never the previous pass's, so running style then
   *  bundle gives the same answer as running bundle once. Only `post.separate.grid`
   *  ({@link Motor.separateNodes}) moves nodes; every other pass leaves `x`/`y` alone. */
  post(handle: Handle, postId: string): PostResult {
    const ctx = this.#requireLoaded();
    const run = runPost(ctx, handle, postId);
    return { handle, id: postId, nodeKind: run.nodeKind, edgeKind: run.edgeKind, nodeCount: nodeCount(ctx.exports, handle), dim: run.dim };
  }

  /** `post.separate.grid`, the one pass that MOVES nodes: read `NodeX`/`NodeY` again after it,
   *  since an earlier view is stale. A 3D snapshot is refused whole ({@link PostRefusedError}). */
  separateNodes(handle: Handle): PostResult {
    return this.post(handle, "post.separate.grid");
  }

  /** Runs the registered analysis `analysisId` (from {@link Motor.analyses}) over
   *  `handle`'s topology and returns its typed result. **No layout run is required**, so this
   *  works straight after {@link Motor.build}; only an unknown handle or an unregistered id
   *  throws. */
  analysis(handle: Handle, analysisId: string): AnalysisResult {
    return runAnalysis(this.#requireLoaded(), handle, analysisId);
  }

  /** The column `columnId` of `handle`'s last run, or `null` if it is reserved or does not
   *  apply to this run's geometry kind (C3) — call {@link Motor.layout} first; a handle with
   *  no successful run yet is refused, not read as "every column absent".
   *
   *  `columnId` is checked against the registered ids *before* it reaches the ABI. `WebAssembly`
   *  coerces an argument to `i32` on the way in, so `motor.column(handle, {} as any)` arrived
   *  as column `0` and was served as `NODE_X` without a word of complaint — the one place in
   *  this SDK that bypassed the `u32` coercion every other argument goes through, and the one
   *  place where coercion is the wrong answer, because a column id is a *name*, not a count. */
  column(handle: Handle, columnId: ColumnId): Column {
    const { views, kinds } = this.#requireLoaded();
    if (!isRegisteredColumn(columnId)) {
      throw new AbiContractError(`column id ${String(columnId)} is not one this ABI registers; see ColumnId`);
    }
    const run = kinds.get(handle);
    if (run === undefined) {
      if (this.#released.has(handle)) {
        throw new InvalidHandleError(`handle ${handle} is not live`, INVALID_HANDLE_CODE);
      }
      throw new InvalidHandleError(`handle ${handle} has no successful run yet`);
    }
    return views.get(toU32(handle) as Handle, columnId, run.nodeKind, run.edgeKind, run.dim);
  }

  /** The canonical JSON face of `handle`'s last run. Refuses with
   *  {@link TamperedGeometryError} if a column view wrote a non-finite value into the motor's
   *  own buffers since that run (D9 re-validation, C8). */
  toJSON(handle: Handle): string {
    const { exports } = this.#requireLoaded();
    return snapshotText(exports, handle);
  }

  /** The binary face of `handle`'s last run. Same D9 re-validation as {@link Motor.toJSON}. */
  toBytes(handle: Handle): Uint8Array {
    const { exports } = this.#requireLoaded();
    return snapshotBytes(exports, handle);
  }

  /** Releases `handle`. Its id is never reissued (C6): using it again after this always
   *  reads {@link InvalidHandleError}, never a different, later graph.
   *
   *  The module's own answer is read, which it was not before: `gm_release` returns `void` and
   *  sets `Code::InvalidHandle` on a refusal, and `invoke` — which inspects a *return word* —
   *  therefore could not see it. Releasing a handle twice, or releasing one this motor never
   *  issued, returned exactly as if it had succeeded, and the second release silently
   *  released the next graph's id instead. A refused release is now
   *  {@link InvalidHandleError} with the recorded code, and the handle stays live: nothing
   *  was released, so nothing is forgotten. */
  release(handle: Handle): void {
    const { exports, views } = this.#requireLoaded();
    invoke("gm_release", () => exports.gm_release(handle) as unknown as number);
    if (lastError(exports) === INVALID_HANDLE_CODE) {
      throw new InvalidHandleError(`handle ${handle} is not live`, INVALID_HANDLE_CODE);
    }
    views.bump();
    views.forget(handle);
    this.#context?.kinds.delete(handle);
    this.#released.add(handle);
  }

  /** Starts a **live force session** over `handle`'s topology
   *  (`docs/decisions/force-wasm-abi.md`), the one surface that runs a layout tick by tick
   *  instead of to a finished picture.
   *
   *  `params` is optional and partial: an omitted field keeps the motor's own value for it, so
   *  a host can move one knob without knowing the other twelve and without a copy of the
   *  defaults in its own source going stale. Every field is range-checked by the motor and
   *  **never clamped**, so an out-of-range value is a refusal with the session left exactly as
   *  it was — and a refused creation leaves no session behind.
   *
   *  **No layout run is required**, exactly as for {@link Motor.analysis}: the session is built
   *  from the topology and seeded on the engine's own spiral. It does not read the graph
   *  handle's snapshot, does not replace it, and **outlives it** — {@link Motor.release} on
   *  `handle` leaves the session running, and the session is released with its own
   *  {@link ForceSession.release}.
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
 *  bound to it. `options` accepts `exec: "auto"` and `threads` (C16, `threads.ts`); any
 *  other shape is refused before the module is even asked to load. */
export async function createMotor(source: WasmSource, options?: MotorOptions): Promise<Motor> {
  return Motor.create(source, options);
}

// The SDK's typed-error policy: every refusal the ABI can report, plus every failure the
// loader itself can hit, is a distinct `GraphMotorError` subclass with a stable `code`
// (mirroring `crates/graph-wasm/src/errors.rs::Code`) and `codeName` (that enum's variant
// name) — never a bare string a caller has to parse, and never a raw
// `WebAssembly.RuntimeError` reaching application code unwrapped.

/** `crates/graph-wasm/src/errors.rs::Code`, by wire value. Append-only, same as the Rust side. */
export const CODE_NAMES = [
  "None",
  "InvalidHandle",
  "AllocFailed",
  "FreeRefused",
  "IngestInvalid",
  "UnknownLayoutId",
  "ParamsMustBeEmpty",
  "HandlesExhausted",
  "LayoutFailed",
  "TamperedGeometry",
  "NoGeometryYet",
  "BuildSourceInvalid",
  "IndexOutOfRange",
  "PostFailed",
  "ContractInvalid",
  "InvalidSession",
  "SessionParamsInvalid",
  "SessionRefused",
  "AnalysisFailed",
  "IngestTooLarge",
  "ParamOutOfRange",
  "ParamsMalformed",
  "ParamsNotAccepted",
  "ColumnsInvalid",
] as const;

/** One `Code`'s name, or `"Unknown(<n>)"` for a wire value this SDK does not know yet —
 * an older SDK reading a newer motor's error code fails loud, never silently as `"None"`. */
export function codeName(code: number): string {
  return CODE_NAMES[code] ?? `Unknown(${code})`;
}

/** Base of every error this package throws. `code`/`codeName` are set on every instance
 * that came from the ABI (`gm_last_error`); the loader's own failures (never reached the
 * ABI at all) leave `code` `undefined` rather than inventing a wire value for them.
 *
 * Every class here writes its `name` as a literal, never `new.target.name`: a minifier renames
 * classes, and the studio's production build reported a refused wasm load as `f` to its host
 * (studio-embed, 2026-10-04). `test/error-names.test.mjs` renames each class to check. */
export class GraphMotorError extends Error {
  override name = "GraphMotorError";
  readonly code: number | undefined;
  readonly codeName: string | undefined;

  constructor(message: string, code?: number) {
    super(message);
    this.code = code;
    this.codeName = code === undefined ? undefined : codeName(code);
  }
}

/** `gm_build` refused the ingest buffer, or the handle table is exhausted. */
export class BuildRefusedError extends GraphMotorError {
  override name = "BuildRefusedError";
}

/** `gm_build_contract` refused the contract document: the strict reader said no
 *  (`ContractInvalid` covers both it and a derivation refusal — a caller debugging a
 *  rejection wants to know the document was refused, not which of the two steps said
 *  so, and `docs/contract/wasm-abi.md` names both), or the handle table is exhausted.
 *
 *  Distinct from {@link BuildRefusedError} rather than a subclass: the two take
 *  different documents, so a caller that catches only this one is saying "a contract
 *  document was refused" and must not silently keep a handle it got from
 *  {@link Motor.build}, whose document means something else. */
export class ContractRefusedError extends GraphMotorError {
  override name = "ContractRefusedError";
}

/** `gm_build_columns` or `gm_graph_extend_columns` refused the columnar document, or the handle
 * table is exhausted. `ColumnsInvalid` is the code for both a *format* fault and a *graph* fault
 *  — a repeated id, an endpoint that names no node — because the columnar path publishes that
 *  one code for both clauses (`docs/contract/wasm-abi.md` "Three build paths").
 *
 *  A sibling of {@link BuildRefusedError} rather than a subclass, and the two are the classes a
 *  caller swapping `extend` for `extendColumns` has to swap with it: the same logical refusal is
 *  `IngestInvalid` under `gm_graph_extend` and `ColumnsInvalid` here, so a host that catches only
 *  `BuildRefusedError` silently loses its error handling on the columnar path. The other
 *  direction is deliberate too — this class is reachable for a document `build` accepts happily
 *  (a repeated node id, which the JSON path drops and the dense-row rule cannot). */
export class ColumnsRefusedError extends GraphMotorError {
  override name = "ColumnsRefusedError";
}

/** `gm_run` refused: an unknown handle, an unknown layout id, or non-empty params
 * (registry layouts take none this phase, C2). */
export class RunRefusedError extends GraphMotorError {
  override name = "RunRefusedError";
}

/** A handle this motor never issued, or already released (`gm_release`, C6). */
export class InvalidHandleError extends GraphMotorError {
  override name = "InvalidHandleError";
}

/** `gm_post_run` refused: an unknown handle, a post id that is not registered, a handle
 *  with no successful layout run to read (there is nothing for a pass to draw over), or
 *  the capability's own failure. Never a silently unchanged drawing. */
export class PostRefusedError extends GraphMotorError {
  override name = "PostRefusedError";
}

/** `gm_analysis_run` refused: an unknown handle, or an analysis id that is not
 *  registered. An analysis never needs a layout run, so `NoGeometryYet` is not among
 *  these — analysing a graph straight after `build` is a supported state. */
export class AnalysisRefusedError extends GraphMotorError {
  override name = "AnalysisRefusedError";
}

/** `gm_snapshot_json`/`gm_snapshot_bytes` refused: a column view wrote a non-finite value
 * into the motor's own buffer since the last run (D9 tamper re-validation, C8). */
export class TamperedGeometryError extends GraphMotorError {
  override name = "TamperedGeometryError";
}

/** The module answered something this ABI forbids: a framed buffer whose length runs past
 *  linear memory, a column `(ptr, len)` pair that is illegal by contract (`ptr === 0` with a
 *  non-zero length, an unaligned `ptr`, or one that overruns the buffer), a column id this
 *  ABI never registered.
 *
 *  A distinct class because none of these is a *refusal* the motor reported — there is no
 *  `gm_last_error` behind them, so `code` is `undefined` — and none is a trap either. Without
 *  it a caller caught a raw `RangeError` from `DataView`/`Float32Array` and could not tell
 *  "the module is broken" from "my own id was wrong". */
export class AbiContractError extends GraphMotorError {
  override name = "AbiContractError";
}

/** `gm_alloc` refused (`AllocFailed`, code 2): the module could not reserve the buffer the
 *  call needed. Distinct from every session's and every stage's own refusal class, because
 *  nothing was asked and nothing refused — the module ran out of room first, and a caller
 *  catching a *session* refusal here would go looking for a bad parameter that is not there. */
export class AllocationFailedError extends GraphMotorError {
  override name = "AllocationFailedError";
}

/** The wasm module trapped (`WebAssembly.RuntimeError`) during a call that should only
 * ever return a sentinel, never trap. Wrapped so it is still a `GraphMotorError`, but
 * `code` is left `undefined`: a trap has no `gm_last_error` behind it. */
export class MotorTrapError extends GraphMotorError {
  override name = "MotorTrapError";
  override readonly cause: unknown;
  constructor(exportName: string, cause: unknown) {
    super(`${exportName} trapped: ${String(cause)}`);
    this.cause = cause;
  }
}

/** `__GM_DISABLE_WASM__` was set, or the module failed to load once already (the
 * `initFailed` latch, `docs/cheatsheet/wasm_native/wasm-bridge.md`): the motor is not
 * coming back this session without an explicit `resetForTests()`. */
export class WasmUnavailableError extends GraphMotorError {
  override name = "WasmUnavailableError";
  readonly reason: unknown;
  constructor(message: string, reason?: unknown) {
    super(message);
    this.reason = reason;
  }
}

/** A caller passed an `options`/params shape this SDK does not recognise (C16's table). */
export class InvalidOptionsError extends GraphMotorError {
  override name = "InvalidOptionsError";
}

/** A live force session id this motor never issued, or one already released
 * (`InvalidSession`). Distinct from {@link InvalidHandleError} rather than a subclass: the two
 * are two id spaces, and a caller catching only this one is saying "my session is gone" —
 * which must not be satisfied by a *graph* handle that happens to be dead. */
export class InvalidSessionError extends GraphMotorError {
  override name = "InvalidSessionError";
}

/** A force session refused the request: a parameter outside its range (**never clamped**), a
 * row past the last node column, or a coordinate that is not finite (`SessionRefused`), or a
 * parameter buffer whose length is neither `0` (the defaults) nor the thirteen-`f64` size
 * (`SessionParamsInvalid`).
 *
 *  A refusal always leaves the session exactly as it was, so catching this and carrying on
 *  costs the caller nothing but the one call. The field and the range are in the motor's own
 *  message text, which the wire does not carry; {@link ForceParams} documents the bounds each
 *  field has, so a host can name the offending one from its own input. */
export class ForceSessionRefusedError extends GraphMotorError {
  override name = "ForceSessionRefusedError";
}

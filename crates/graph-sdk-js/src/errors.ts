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
  "ParamOutOfRange",
  "ParamsMalformed",
  "ParamsNotAccepted",
] as const;

/** One `Code`'s name, or `"Unknown(<n>)"` for a wire value this SDK does not know yet —
 * an older SDK reading a newer motor's error code fails loud, never silently as `"None"`. */
export function codeName(code: number): string {
  return CODE_NAMES[code] ?? `Unknown(${code})`;
}

/** Base of every error this package throws. `code`/`codeName` are set on every instance
 * that came from the ABI (`gm_last_error`); the loader's own failures (never reached the
 * ABI at all) leave `code` `undefined` rather than inventing a wire value for them. */
export class GraphMotorError extends Error {
  readonly code: number | undefined;
  readonly codeName: string | undefined;

  constructor(message: string, code?: number) {
    super(message);
    this.name = new.target.name;
    this.code = code;
    this.codeName = code === undefined ? undefined : codeName(code);
  }
}

/** `gm_build` refused the ingest buffer, or the handle table is exhausted. */
export class BuildRefusedError extends GraphMotorError {}

/** `gm_build_contract` refused the contract document: the strict reader said no
 *  (`ContractInvalid` covers both it and a derivation refusal — a caller debugging a
 *  rejection wants to know the document was refused, not which of the two steps said
 *  so, and `docs/contract/wasm-abi.md` names both), or the handle table is exhausted.
 *
 *  Distinct from {@link BuildRefusedError} rather than a subclass: the two take
 *  different documents, so a caller that catches only this one is saying "a contract
 *  document was refused" and must not silently keep a handle it got from
 *  {@link Motor.build}, whose document means something else. */
export class ContractRefusedError extends GraphMotorError {}

/** `gm_run` refused: an unknown handle, an unknown layout id, or non-empty params
 * (registry layouts take none this phase, C2). */
export class RunRefusedError extends GraphMotorError {}

/** A handle this motor never issued, or already released (`gm_release`, C6). */
export class InvalidHandleError extends GraphMotorError {}

/** `gm_post_run` refused: an unknown handle, a post id that is not registered, a handle
 *  with no successful layout run to read (there is nothing for a pass to draw over), or
 *  the capability's own failure. Never a silently unchanged drawing. */
export class PostRefusedError extends GraphMotorError {}

/** `gm_analysis_run` refused: an unknown handle, or an analysis id that is not
 *  registered. An analysis never needs a layout run, so `NoGeometryYet` is not among
 *  these — analysing a graph straight after `build` is a supported state. */
export class AnalysisRefusedError extends GraphMotorError {}

/** `gm_snapshot_json`/`gm_snapshot_bytes` refused: a column view wrote a non-finite value
 * into the motor's own buffer since the last run (D9 tamper re-validation, C8). */
export class TamperedGeometryError extends GraphMotorError {}

/** The wasm module trapped (`WebAssembly.RuntimeError`) during a call that should only
 * ever return a sentinel, never trap. Wrapped so it is still a `GraphMotorError`, but
 * `code` is left `undefined`: a trap has no `gm_last_error` behind it. */
export class MotorTrapError extends GraphMotorError {
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
  readonly reason: unknown;
  constructor(message: string, reason?: unknown) {
    super(message);
    this.reason = reason;
  }
}

/** A caller passed an `options`/params shape this SDK does not recognise (C16's table). */
export class InvalidOptionsError extends GraphMotorError {}

/** A live force session id this motor never issued, or one already released
 * (`InvalidSession`). Distinct from {@link InvalidHandleError} rather than a subclass: the two
 * are two id spaces, and a caller catching only this one is saying "my session is gone" —
 * which must not be satisfied by a *graph* handle that happens to be dead. */
export class InvalidSessionError extends GraphMotorError {}

/** A force session refused the request: a parameter outside its range (**never clamped**), a
 * row past the last node column, or a coordinate that is not finite (`SessionRefused`), or a
 * parameter buffer whose length is neither `0` (the defaults) nor the thirteen-`f64` size
 * (`SessionParamsInvalid`).
 *
 *  A refusal always leaves the session exactly as it was, so catching this and carrying on
 *  costs the caller nothing but the one call. The field and the range are in the motor's own
 *  message text, which the wire does not carry; {@link ForceParams} documents the bounds each
 *  field has, so a host can name the offending one from its own input. */
export class ForceSessionRefusedError extends GraphMotorError {}

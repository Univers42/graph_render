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

/** `gm_run` refused: an unknown handle, an unknown layout id, or non-empty params
 * (registry layouts take none this phase, C2). */
export class RunRefusedError extends GraphMotorError {}

/** A handle this motor never issued, or already released (`gm_release`, C6). */
export class InvalidHandleError extends GraphMotorError {}

/** `gm_snapshot_json`/`gm_snapshot_bytes` refused: a column view wrote a non-finite value
 * into the motor's own buffer since the last run (D9 tamper re-validation, C8). */
export class TamperedGeometryError extends GraphMotorError {}

/** The wasm module trapped (`WebAssembly.RuntimeError`) during a call that should only
 * ever return a sentinel, never trap. Wrapped so it is still a `GraphMotorError`, but
 * `code` is left `undefined`: a trap has no `gm_last_error` behind it. */
export class MotorTrapError extends GraphMotorError {
  readonly cause: unknown;
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

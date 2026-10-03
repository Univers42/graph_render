// The wire's thirteen force parameters, in one place (`docs/decisions/force-wasm-abi.md`):
// the declaration order `crates/graph-wasm/src/session/params.rs` encodes and decodes, the
// 104-byte staging buffer they cross the ABI in, and the merge a partial `setParams` makes.
//
// Split out of `force.ts` so that file holds the session and this one holds the wire, which
// is what both of them are actually about: a field added on the Rust side and missing from
// `FIELDS` shows up as a length mismatch here rather than as a silently shifted parameter.

import { AllocationFailedError, ForceSessionRefusedError } from "./errors.ts";
import { SESSION_PARAMS_INVALID_CODE, SESSION_REFUSED_CODE, invoke, lastError } from "./calls.ts";
import { type RawExports } from "./wasm.ts";
import type { ForceParams } from "./types.ts";

/** The wire's thirteen parameter fields, in `LiveParams`' declaration order. One list, read
 *  in both directions, so the two cannot drift. */
const FIELDS = [
  "charge",
  "theta",
  "distance_min",
  "distance_max",
  "link_distance",
  "link_strength_scale",
  "collide_radius",
  "center_strength",
  "gravity",
  "velocity_decay",
  "alpha_decay",
  "alpha_min",
  "initial_alpha",
] as const satisfies readonly (keyof ForceParams)[];

/** The parameter buffer's byte length: thirteen little-endian `f64`s. */
export const PARAMS_BYTES = FIELDS.length * 8;

/** The largest value `toU32` will carry across the ABI, and the largest a `u32` field of a
 *  contract document or a row index can hold. Named once because four call sites in this
 *  package refuse on exactly this bound and a spelled-out `4294967295` four times is four
 *  chances to spell it wrong. */
export const U32_MAX = 0xffffffff;

/** Whether `value` is a word this ABI accepts as a `u32` count or index: an integer, not
 *  negative, not past 2^32-1. `toU32` alone is not this test — `NaN >>> 0` and
 *  `-1 >>> 0` are `0` and `4294967295`, both legal words, which is exactly how a `NaN` row
 *  became row 0 and a negative tick count became four billion ticks. */
export function isU32(value: number): boolean {
  return Number.isInteger(value) && value >= 0 && value <= U32_MAX;
}

/** `params` as the wire's 104 bytes: thirteen little-endian `f64`s in `FIELDS` order.
 *
 *  Written through a `DataView` rather than a `Float64Array` over the staging buffer, because
 *  `gm_alloc` hands out 4-aligned memory and a `Float64Array` view at a 4-aligned offset
 *  throws. The motor reads the same buffer bytewise for the same reason. */
export function encodeParams(params: ForceParams): Uint8Array {
  const bytes = new Uint8Array(PARAMS_BYTES);
  const view = new DataView(bytes.buffer);
  FIELDS.forEach((field, i) => view.setFloat64(i * 8, params[field], true));
  return bytes;
}

/** The 104 bytes of a framed `gm_force_session_params` buffer as a typed set. A buffer of any
 *  other length is `SessionParamsInvalid` rather than a partly-read set. */
export function decodeParams(bytes: Uint8Array): ForceParams {
  if (bytes.length !== PARAMS_BYTES) {
    throw new ForceSessionRefusedError(
      `a parameter buffer is ${String(PARAMS_BYTES)} bytes, got ${String(bytes.length)}`,
      SESSION_PARAMS_INVALID_CODE,
    );
  }
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const out: Record<string, number> = {};
  FIELDS.forEach((field, i) => {
    out[field] = view.getFloat64(i * 8, true);
  });
  return out as unknown as ForceParams;
}

/** `current` with every field `params` actually names, taken from `params`.
 *
 *  A field the caller *omitted* keeps the motor's own value. A field the caller passed as
 *  `undefined` is omitted too, and that is the whole point: a spread merge
 *  (`{ ...current, ...params }`) cannot tell the two apart, so an `undefined` written by a
 *  spread-constructed object, a destructuring default that resolved to `undefined`, or a
 *  `Partial` built from a form field the user never filled in silently overwrote a real
 *  value with `undefined`, which reached the wire as `NaN` and came back as a refusal naming
 *  a field the caller was confident they had said nothing about. Only the thirteen named
 *  fields are read, so a misspelled key is dropped exactly as it was before. */
export function mergeParams(current: ForceParams, params: Partial<ForceParams>): ForceParams {
  const merged = { ...current } as Record<string, number>;
  for (const field of FIELDS) {
    const asked = params[field];
    if (asked !== undefined) merged[field] = asked;
  }
  return merged as unknown as ForceParams;
}

/** One `u32`-shaped argument this SDK is about to send, or a refusal naming it. A `u32`
 *  count or index is exact arithmetic the caller's own value must survive (C9), so this is
 *  the check that has to come *before* `toU32`, never after — `toU32` alone turns `-1` into
 *  `4294967295` and `NaN` into `0`, both legal words and both the wrong number.
 *
 *  A {@link ForceSessionRefusedError}, because that is the class already scoped to "a parameter
 *  outside its range, never clamped": a negative tick count is the same kind of mistake as a
 *  charge outside `-5000..=0`, caught one layer earlier so the count cannot reach the
 *  integrator. The recorded code is `SessionRefused`, not `SessionParamsInvalid` — the latter
 *  is about a buffer of the wrong *length*, and nothing here has a buffer. */
export function asU32(value: number, what: string): number {
  if (!isU32(value)) {
    throw new ForceSessionRefusedError(
      `${what} must be an integer in 0..${String(U32_MAX)} (a u32), got ${String(value)}`,
      SESSION_REFUSED_CODE,
    );
  }
  return value;
}

/** Stages `params` in linear memory, hands the address to `send`, and frees it again — on a
 *  refusal too. C7: the staging buffer is this function's job, since the caller never sees the
 *  pointer, so a `setParams` that is refused must not leave 104 bytes reserved in the module.
 *
 *  `gm_alloc` refusing is an {@link AllocationFailedError}, not a session refusal: no session
 *  was asked anything, and a caller catching `ForceSessionRefusedError` here would go looking
 *  for an out-of-range parameter that is not there. */
export function withStagedParams(exports: RawExports, params: ForceParams, send: (ptr: number) => void): void {
  const bytes = encodeParams(params);
  const ptr = invoke("gm_alloc", () => exports.gm_alloc(PARAMS_BYTES));
  if (ptr === 0) throw new AllocationFailedError("gm_alloc could not reserve the parameter buffer", lastError(exports));
  new Uint8Array(exports.memory.buffer, ptr, PARAMS_BYTES).set(bytes);
  try {
    send(ptr);
  } finally {
    invoke("gm_free", () => exports.gm_free(ptr, PARAMS_BYTES));
  }
}

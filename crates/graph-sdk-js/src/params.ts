// One layout's published parameters, and the run buffer built from them, over the motor's
// own ABI calls: `gm_layout_params`, and the staging of `gm_run`'s parameter buffer
// (`docs/decisions/layout-params.md`).
//
// Split out of `index.ts` by the house's 300-line limit: the typed `Motor` surface stays
// there and re-exports everything here, so the published entry point's exports are
// unchanged. What moved is the *buffer*, not the decision about it — `index.ts` still says
// which layout and which values, and this module is the only place a `gm_alloc`ed
// parameter buffer exists.

import { toU32, type RawExports } from "./wasm.ts";
import { RunRefusedError, codeName } from "./errors.ts";
import { frame, invoke, lastError } from "./calls.ts";
import type { ColumnViews } from "./views.ts";
import type { Handle } from "./types.ts";
import { decodeLayoutParams, encodeLayoutParams, type LayoutParamSpec } from "./layout-params.ts";
import { runRefusal } from "./stages.ts";

export * from "./layout-params.ts";

/** One run's parameters: the layout to draw, and the values to draw it at. `values`
 *  `undefined` means "no opinion" — the empty buffer, which is the layout's own defaults,
 *  and what every caller before ABI 2 sent. */
export interface RunAtParams {
  /** The graph to draw. */
  readonly handle: Handle;
  /** `gm_run`'s `layout_id`: the registry index `layoutId` resolved to. */
  readonly layoutIndex: number;
  /** The schema `values` is keyed by. Empty only when `values` is `undefined`. */
  readonly specs: readonly LayoutParamSpec[];
  /** What the caller asked for, by published name; `undefined` for the defaults. */
  readonly values: Readonly<Record<string, number | boolean>> | undefined;
}

/** One layout's published parameters per id, read once per motor.
 *
 *  Its own object rather than a `Map` field on `Motor` so the cache and the two ABI calls
 *  that fill it sit together, and so a second `Motor` cannot share one by accident: a
 *  schema belongs to a live module, and a module belongs to a motor. */
export class LayoutParams {
  readonly #byId = new Map<string, LayoutParamSpec[]>();

  /** What `layoutId` publishes, in buffer order, cached after the first read.
   *
   *  An empty array is an answer, not a refusal: most registered layouts pin their own
   *  conventions and publish nothing. A body this SDK cannot read is a `RangeError` from
   *  the decoder rather than an empty array, because "this module speaks a different ABI"
   *  must not read as "this layout takes no parameters". */
  read(exports: RawExports, layoutId: string, layoutIndex: number): LayoutParamSpec[] {
    const cached = this.#byId.get(layoutId);
    if (cached !== undefined) return cached;
    const ptr = invoke("gm_layout_params", () => exports.gm_layout_params(toU32(layoutIndex)));
    if (ptr === 0) {
      throw new RunRefusedError(`gm_layout_params(${layoutId}) refused (${codeName(lastError(exports))})`);
    }
    const specs = decodeLayoutParams(frame(exports, ptr));
    this.#byId.set(layoutId, specs);
    return specs;
  }
}

/** Runs `gm_run` at `run`'s values, or at the layout's own defaults when `run.values` is
 *  `undefined`.
 *
 *  The buffer is staged through `gm_alloc` and freed here (C7): the caller never sees a
 *  pointer, and a refusal frees it on the way out rather than leaking it. `views.bump()`
 *  brackets the call exactly as the unstaged path did, so the zero-copy epoch moves
 *  whether or not there was a buffer to move. */
export function runAtParams(exports: RawExports, views: ColumnViews, run: RunAtParams): void {
  const bytes = run.values === undefined ? new Uint8Array(0) : encodeLayoutParams(run.specs, run.values);
  const len = toU32(bytes.byteLength);
  const ptr = len === 0 ? 0 : stageParams(exports, bytes, len);
  try {
    const ok = invoke("gm_run", () => exports.gm_run(toU32(run.handle), toU32(run.layoutIndex), toU32(ptr), len));
    views.bump();
    if (ok !== 1) throw runRefusal(run.handle, lastError(exports));
  } finally {
    if (len !== 0) {
      invoke("gm_free", () => exports.gm_free(toU32(ptr), len));
      views.bump();
    }
  }
}

/// The parameter buffer in linear memory, or the refusal. `len` is the buffer's own length
/// and `0` is never a valid pointer here, because `runAtParams` stages only a non-empty
/// buffer — and `gm_alloc` returning `0` is `AllocFailed`, not an address at zero.
function stageParams(exports: RawExports, bytes: Uint8Array, len: number): number {
  const ptr = invoke("gm_alloc", () => exports.gm_alloc(len));
  if (ptr === 0) {
    throw new RunRefusedError(`gm_alloc could not reserve the params buffer (${codeName(lastError(exports))})`, lastError(exports));
  }
  new Uint8Array(exports.memory.buffer, ptr, len).set(bytes);
  return ptr;
}
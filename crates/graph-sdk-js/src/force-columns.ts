// The two position columns of a live force session, and D9's gate in front of them.
//
// This is a separate module because a session's columns are a *separate* problem from the
// session's verbs, and because they are the one place in this SDK where the caller is handed
// a writable window over the motor's own simulation state with no snapshot face behind it:
// `gm_snapshot_json`'s tamper re-check cannot reach a session, and `sim.rs` checks nothing.
// So the pair is validated on the way out and the contents are re-validated on every read and
// every tick, by one owner, and a caller cannot get a view that has not passed both.

import { AbiContractError, ForceSessionRefusedError, InvalidSessionError, TamperedGeometryError, codeName } from "./errors.ts";
import { INVALID_SESSION_CODE, invoke, lastError, type Loaded } from "./calls.ts";
import { toU32, type RawExports } from "./wasm.ts";
import type { ForceSessionId } from "./types.ts";

/** The `axis` argument of the two column calls that names the `x` column. */
export const X_AXIS = 0;
/** …and the one that names `y`. */
export const Y_AXIS = 1;
/** The `axis` argument of the two velocity calls that names the `vx` column. */
export const VX_AXIS = 0;
/** …and the one that names `vy`. */
export const VY_AXIS = 1;

/** One `Float64Array` over the session's own storage, or a refusal when the pair is illegal.
 *
 *  A `Vec<f64>`'s address is 8-aligned by construction, so the `Float64Array` constructor's
 *  own alignment requirement is met; 4 is the bound that matters here, since 4 is what the
 *  wire's `u32` addresses are and what the snapshot columns in `views.ts` are checked against.
 *  `0` is the motor refusing and a `0` length is a graph with no nodes, so `(0, anything)` is
 *  refused rather than handed back as an empty column a renderer would read as "everything is
 *  at the origin". */
function checkedColumn(buffer: ArrayBufferLike, address: number, len: number, axis: number, kind = "position"): Float64Array {
  const named = `the ${kind} column (axis ${String(axis)})`;
  if (address === 0) throw new AbiContractError(`${named} answered address 0, which is never a column's address`);
  if (address % 4 !== 0) throw new AbiContractError(`${named} answered address ${String(address)}, which is not 4-aligned`);
  if (len <= 0) throw new AbiContractError(`${named} is empty, so there is nothing to read`);
  if (address + len * Float64Array.BYTES_PER_ELEMENT > buffer.byteLength) {
    throw new AbiContractError(`${named} answered (${String(address)}, ${String(len)}), past the end of ${String(buffer.byteLength)} bytes`);
  }
  return new Float64Array(buffer, address, len);
}

/** D9, for the one surface that has no snapshot face: nothing else re-checks a force session's
 *  positions, so a `NaN` written through a view integrates into every later tick silently.
 *
 *  Refused, loudly and permanently, the same way a tampered *handle* is:
 *  `TamperedGeometryError`, no clamping, no repair. A session whose positions went non-finite
 *  has no honest state left to return to, and quietly substituting a finite value would be a
 *  graph nobody drew.
 *
 *  Ponytail: this is two linear scans per call, on the interactive loop's hot path. Failing
 *  input: a 200k-node session driven one batch per frame pays two passes over 3.2 MB it did
 *  not otherwise have to. Direction: refuse rather than trust, because the alternative is a
 *  NaN that spreads one tick further per frame and is still there when the frame ends. Escape
 *  hatch: none, on purpose — the flag would be off exactly where it is cheapest to leave it
 *  off, and the honest escape hatch is `pin`, which validates on its own way in. */
export function assertFinitePositions(id: ForceSessionId, columns: readonly Float64Array[]): void {
  for (const column of columns) {
    for (let row = 0; row < column.length; row += 1) {
      if (Number.isFinite(column[row] as number)) continue;
      throw new TamperedGeometryError(
        `force session ${String(id)}: row ${String(row)} holds a non-finite position, written through the zero-copy view`,
      );
    }
  }
}

/** D9 for the velocity columns, which nothing else re-checks either: the same two scans, the
 *  same permanent refusal, and the same reason — a `NaN` velocity integrates into every later
 *  tick exactly as a `NaN` position does. */
export function assertFiniteVelocities(id: ForceSessionId, columns: readonly Float64Array[]): void {
  for (const column of columns) {
    for (let row = 0; row < column.length; row += 1) {
      if (Number.isFinite(column[row] as number)) continue;
      throw new TamperedGeometryError(
        `force session ${String(id)}: row ${String(row)} holds a non-finite velocity, written through the zero-copy view`,
      );
    }
  }
}

/** One session's two position columns: read them, keep the views fresh, and stand in front of
 *  them. {@link ForceSession} owns one of these and is the only thing that constructs it.
 *
 *  Liveness is *not* this class's business: it reports a session the module has dropped as
 *  `InvalidSessionError`, and the session turns that into its own released state, so the flag
 *  that says "this session is gone" lives in exactly one place. */
export class ForceColumns {
  readonly #loaded: Loaded;
  readonly #id: ForceSessionId;
  /** The last two position views, dropped whenever wasm memory itself has been replaced. */
  #views: { readonly buffer: ArrayBufferLike; readonly xs: Float64Array; readonly ys: Float64Array } | null = null;
  /** The last two velocity views, on their own cache: a mesh tick swaps them with its scratch
   *  for the same reason it swaps the positions, so they go stale on the same two events and
   *  are re-derived on the next call either way. */
  #velocities: { readonly buffer: ArrayBufferLike; readonly vxs: Float64Array; readonly vys: Float64Array } | null = null;

  constructor(loaded: Loaded, id: ForceSessionId) {
    this.#loaded = loaded;
    this.#id = id;
  }

  /** Both columns, zero-copy, re-derived when the backing `ArrayBuffer` has been replaced (a
   *  growth detaches the old one, which is silent garbage rather than an error) and
   *  finiteness-checked on the way out. */
  read(): { readonly xs: Float64Array; readonly ys: Float64Array } {
    const { exports } = this.#loaded;
    const cached = this.#views;
    // The cached path is checked too, and it is the *common* one: a caller that reads a frame,
    // writes through it, and reads the next frame without ever ticking arrives here, and a
    // gate that only ran on the first derivation would miss exactly the write it exists for.
    if (cached !== null && cached.buffer === exports.memory.buffer) {
      assertFinitePositions(this.#id, [cached.xs, cached.ys]);
      return { xs: cached.xs, ys: cached.ys };
    }
    const xs = this.#column(exports, X_AXIS);
    const ys = this.#column(exports, Y_AXIS);
    assertFinitePositions(this.#id, [xs, ys]);
    this.#views = { buffer: exports.memory.buffer, xs, ys };
    return { xs, ys };
  }

  /** Both velocity columns, zero-copy, re-derived when the backing `ArrayBuffer` has been
   *  replaced and finiteness-checked on the way out — the same contract {@link read} gives the
   *  positions, and for the same reason: a mesh tick swaps `sim.vx`/`sim.vy` with its scratch,
   *  so a held view is stale after a tick exactly as a position view is. */
  readVelocities(): { readonly vxs: Float64Array; readonly vys: Float64Array } {
    const { exports } = this.#loaded;
    const cached = this.#velocities;
    // The cached path is checked too, and it is the *common* one, for the reason `read` gives:
    // a caller that reads a frame, writes through it, and reads the next frame without ever
    // ticking arrives here.
    if (cached !== null && cached.buffer === exports.memory.buffer) {
      assertFiniteVelocities(this.#id, [cached.vxs, cached.vys]);
      return { vxs: cached.vxs, vys: cached.vys };
    }
    const vxs = this.#velocity(exports, VX_AXIS);
    const vys = this.#velocity(exports, VY_AXIS);
    assertFiniteVelocities(this.#id, [vxs, vys]);
    this.#velocities = { buffer: exports.memory.buffer, vxs, vys };
    return { vxs, vys };
  }

  /** The D9 gate on its own, so `tick` and `read` cannot disagree about when it runs. The
   *  views are not cached here: a tick moves the columns, so a gate that trusted a cached
   *  view would be checking the frame before the tick rather than the one about to be read. */
  assertFinite(): void {
    const { exports } = this.#loaded;
    assertFinitePositions(this.#id, [this.#column(exports, X_AXIS), this.#column(exports, Y_AXIS)]);
    assertFiniteVelocities(this.#id, [this.#velocity(exports, VX_AXIS), this.#velocity(exports, VY_AXIS)]);
  }

  /** Forgets the cached views — after a tick, which swaps the position and velocity columns in,
   *  and after a release, so a session cannot hand back a view over memory it no longer owns. */
  forget(): void {
    this.#views = null;
    this.#velocities = null;
  }

  #column(exports: RawExports, axis: number): Float64Array {
    const id = toU32(this.#id);
    const address = invoke("gm_force_session_column_ptr", () =>
      exports.gm_force_session_column_ptr(id, toU32(axis)),
    );
    const len = invoke("gm_force_session_column_len", () => exports.gm_force_session_column_len(id, toU32(axis)));
    if (address !== 0 && len > 0) return checkedColumn(exports.memory.buffer, address, len, axis);
    const code = lastError(exports);
    if (code === INVALID_SESSION_CODE) {
      throw new InvalidSessionError(`force session ${String(this.#id)} is not live`, INVALID_SESSION_CODE);
    }
    throw new ForceSessionRefusedError(`the position column (axis ${axis}) is not readable (${codeName(code)})`, code);
  }

  /** One velocity column, checked and named the same way `#column` is — the velocity exports
   *  are the same `(ptr, len)` pair with the same two refusals. */
  #velocity(exports: RawExports, axis: number): Float64Array {
    const id = toU32(this.#id);
    const address = invoke("gm_force_session_velocity_ptr", () =>
      exports.gm_force_session_velocity_ptr(id, toU32(axis)),
    );
    const len = invoke("gm_force_session_velocity_len", () => exports.gm_force_session_velocity_len(id, toU32(axis)));
    if (address !== 0 && len > 0) return checkedColumn(exports.memory.buffer, address, len, axis, "velocity");
    const code = lastError(exports);
    if (code === INVALID_SESSION_CODE) {
      throw new InvalidSessionError(`force session ${String(this.#id)} is not live`, INVALID_SESSION_CODE);
    }
    throw new ForceSessionRefusedError(`the velocity column (axis ${axis}) is not readable (${codeName(code)})`, code);
  }
}

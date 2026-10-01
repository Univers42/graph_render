// The typed force session over `gm_force_session_*` (`docs/decisions/force-wasm-abi.md`):
// one `ForceSession` per live simulation, driven tick by tick. The physics is graph-core's
// `ForceSession` behind the wasm ABI; this file adds no arithmetic of its own — every method is
// a typed call, a `u32` coercion (C9), and a refusal turned into a `GraphMotorError` subclass
// rather than a bare `0`.
//
// Two decisions here that the wire's table records and this file is the other half of:
//
// - **Positions are `Float64Array`, not `Float32Array`.** Every other column this SDK hands
//   out is `f32`, because every other column is a *snapshot* column. A live session's columns
//   are the simulation state the next tick reads back, so narrowing them here would lose
//   precision the simulation then integrates from.
// - **The parameter defaults are read from the motor, never written here.** `params()` asks
//   `gm_force_session_params` for the session's own thirteen values, so a partial
//   `setParams` means "these fields, the motor's own value for the rest" and no copy of
//   `LiveParams::default` can go stale in this package.

import { toU32, type RawExports } from "./wasm.ts";
import { ForceSessionRefusedError, InvalidSessionError, codeName } from "./errors.ts";
import {
  INVALID_SESSION_CODE,
  SESSION_PARAMS_INVALID_CODE,
  frame,
  invoke,
  lastError,
  type Loaded,
} from "./calls.ts";
import type { ForceParams, ForceSessionId, ForceTick, Handle } from "./types.ts";

/** The wire's thirteen parameter fields, in `LiveParams`' declaration order — the order
 *  `crates/graph-wasm/src/session/params.rs` encodes and decodes. One list, read in both
 *  directions here, so a field added on the Rust side and missing here shows up as a length
 *  mismatch rather than as a silently shifted parameter. */
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

/** The `axis` argument of the two column calls that names the `x` column. */
const X_AXIS = 0;
/** …and the one that names `y`. */
const Y_AXIS = 1;

/** A live force simulation over one graph, driven by the caller
 *  (`docs/decisions/force-wasm-abi.md`). {@link Motor.forceSession} is the only way to get one.
 *
 *  A session **outlives its graph handle**: `gm_force_session_create` copies what it needs out
 *  of the topology and keeps no borrow, so {@link Motor.release} on that graph does not disturb
 *  it. The two also have separate id spaces and separate error codes, so releasing a session is
 *  a separate call ({@link ForceSession.release}) — holding one is cheap, leaking one is not. */
export class ForceSession {
  readonly #loaded: Loaded;
  readonly #graph: Handle;
  readonly #id: ForceSessionId;
  #live = true;
  /** The last two position views, re-derived whenever wasm memory itself has been replaced. */
  #views: { readonly buffer: ArrayBufferLike; readonly xs: Float64Array; readonly ys: Float64Array } | null = null;

  /** @internal — use {@link Motor.forceSession}. Never throws for a load failure (the motor
   *  has already been asked, and this only reaches the ABI once it has answered): every refusal
   *  below is a typed {@link GraphMotorError}, and a refused creation leaves no session behind. */
  constructor(loaded: Loaded, graph: Handle, params?: Partial<ForceParams>) {
    this.#loaded = loaded;
    this.#graph = graph;
    this.#id = this.#create(graph);
    if (params === undefined) return;
    try {
      this.setParams(params);
    } catch (error) {
      // A refusal here would otherwise leave a session this caller never received a handle to.
      this.release();
      throw error;
    }
  }

  /** This session's id, for a host keeping its own map of them. Never reissued after
   *  {@link ForceSession.release} (C6). */
  get id(): ForceSessionId {
    return this.#id;
  }

  /** The graph handle this session was created from. The session does not need it any more; this
   *  is here so a host can name which graph a session belongs to. */
  get graph(): Handle {
    return this.#graph;
  }

  /** Whether {@link ForceSession.release} has been called, or a call found the session already
   *  gone. Every method below refuses a dead session rather than answering emptily. */
  get released(): boolean {
    return !this.#live;
  }

  /** Runs `ticks` ticks and says what happened. **Synchronous and bounded**: the whole batch
   *  runs before this returns, which is what an interactive loop wants (one batch per frame)
   *  and what a worker wants (the frame cannot be torn in half).
   *
   *  `ticks` is the only argument that changes the result: `tick(112)`, `112` calls of
   *  `tick(1)` and `7` of `tick(16)` are the same positions — graph-core's guarantee, and the
   *  reason a batch settle and an interactive loop share one code path.
   *
   *  Pins and parameters placed before this are what the ticks see: a verb moves nothing on its
   *  own, so a drag reads its effect on the **next** tick. */
  tick(ticks: number): ForceTick {
    const word = this.#call("gm_force_session_tick", (exports) =>
      exports.gm_force_session_tick(toU32(this.#id), toU32(ticks)),
    );
    if (word !== 1 && word !== 2) {
      throw new ForceSessionRefusedError(`gm_force_session_tick answered ${String(word)}, which is not a status`);
    }
    const report: ForceTick = {
      status: word === 2 ? "settled" : "running",
      alpha: this.alpha,
      ticksRun: ticks,
    };
    return report;
  }

  /** The cooling schedule's current value, as the last tick left it. `0` is both a legal value
   *  and the wire's refusal value, so this resolves the ambiguity through `gm_last_error` (C4)
   *  rather than handing back a `0` a caller would read as a cold layout. */
  get alpha(): number {
    this.#requireLive();
    const { exports } = this.#loaded;
    const alpha = invoke("gm_force_session_alpha", () =>
      exports.gm_force_session_alpha(toU32(this.#id)),
    );
    if (lastError(exports) === INVALID_SESSION_CODE) throw this.#dead();
    return alpha;
  }

  /** Holds the node at `row` at `(x, y)` exactly, from the next tick on.
   *
   *  `row` is the node's row in the two position columns — the dense order
   *  `motor.column(handle, ColumnId.NodeX)` already addresses. No node-id string crosses the
   *  ABI, so a host holding ids maps them itself (that is what the studio's drag port does).
   *
   *  A row past the last node, or a coordinate that is not finite, is refused with the session
   *  untouched: a pin that silently did nothing would be a drag the user cannot explain. */
  pin(row: number, x: number, y: number): void {
    this.#call("gm_force_session_pin", (exports) =>
      exports.gm_force_session_pin(toU32(this.#id), toU32(row), x, y),
    );
  }

  /** {@link ForceSession.pin} under the name an interactive host uses it by: **a drag is a pin
   *  at a new point**, and the motor has one call for both (d3's `node.fx`/`node.fy`). Both
   *  names are here because both ports that drive this SDK say one of them, and the
   *  alternative is a translation layer forwarding to the method beside it. */
  drag(row: number, x: number, y: number): void {
    this.pin(row, x, y);
  }

  /** Releases one row, which then integrates again from rest. Releasing a row that was never
   *  pinned is not an error: it is what "let go of this node" means when nothing held it. */
  unpin(row: number): void {
    this.#call("gm_force_session_unpin", (exports) =>
      exports.gm_force_session_unpin(toU32(this.#id), toU32(row)),
    );
  }

  /** Releases every row at once: what a host does when it stops driving the loop, so no pin
   *  survives into the next run. */
  unpinAll(): void {
    this.#call("gm_force_session_unpin_all", (exports) =>
      exports.gm_force_session_unpin_all(toU32(this.#id)),
    );
  }

  /** Moves every row to where this session's graph was last drawn (its snapshot, after any
   *  post pass), with velocities zeroed; pins and alpha are kept. Call it after each layout, so
   *  the next drag moves the picture on screen rather than the session's own seed. Refused with
   *  a typed error before the graph's first run. */
  seat(): void {
    this.#call("gm_force_session_seat", (exports) =>
      exports.gm_force_session_seat(toU32(this.#id), toU32(this.#graph)),
    );
  }

  /** Moves every row back to the spiral a new session starts on, at rest, at the starting
   *  alpha: the settle starts over from the motor's own seed. Parameters and pins are kept. */
  restart(): void {
    this.#call("gm_force_session_restart", (exports) =>
      exports.gm_force_session_restart(toU32(this.#id)),
    );
  }

  /** Sets `alpha` outright — the verb behind "the user moved something, run it again". Taken
   *  exactly and **never clamped**: `0..=1`, and anything else is refused with the session left
   *  as it was, because a clamp would be a lie the caller cannot see. */
  reheat(alpha: number): void {
    this.#call("gm_force_session_reheat", (exports) =>
      exports.gm_force_session_reheat(toU32(this.#id), alpha),
    );
  }

  /** Replaces some or all of the session's parameters.
   *
   *  A field the caller omits keeps **the motor's own current value**, read back through
   *  `gm_force_session_params` rather than from a copy of the defaults in this package — so a
   *  host can send one knob without knowing the other twelve, and cannot send the wrong twelve.
   *  Every field is range-checked and never clamped; any one of them out of range refuses the
   *  whole call and changes nothing.
   *
   *  Only the per-link geometry that depends on the parameters is recomputed: positions and
   *  velocities are not reset, which is what makes this a knob and not a restart. */
  setParams(params: Partial<ForceParams>): void {
    const merged: ForceParams = { ...this.params(), ...params };
    this.#staged(merged, (exports, ptr) =>
      this.#call("gm_force_session_set_params", () =>
        exports.gm_force_session_set_params(toU32(this.#id), ptr, PARAMS_BYTES),
      ),
    );
  }

  /** The parameters in force, field for field, as the motor holds them. */
  params(): ForceParams {
    const ptr = this.#call("gm_force_session_params", (exports) =>
      exports.gm_force_session_params(toU32(this.#id)),
    );
    return decodeParams(frame(this.#loaded.exports, ptr));
  }

  /** The two position columns, one `Float64Array` each, one entry per node in row order.
   *
   *  **Zero-copy**: both views alias the session's own columns, so this is the cheapest way to
   *  read a frame, and they are re-derived whenever wasm memory itself has been replaced since
   *  the last call (a growth detaches the old `ArrayBuffer`, which is silent garbage rather
   *  than an error). The address is stable for the session's life: the motor never resizes
   *  those columns, and each session lives behind a `Box` so no later insert moves it.
   *
   *  Ponytail: a held view is only as fresh as the last tick. Copy it (`.slice()`) before
   *  transferring it to a worker or keeping it past the next call, exactly as this SDK's column
   *  views must be treated (C7). Failing input: a host that stores the returned arrays and
   *  reads them after driving another session — the values are live and will have moved.
   *  Direction: reading the freshest values, which is what a renderer wants. Escape hatch:
   *  call this method again per frame; it is two pointer reads. */
  positions(): { readonly xs: Float64Array; readonly ys: Float64Array } {
    const { exports } = this.#loaded;
    const cached = this.#views;
    if (cached !== null && cached.buffer === exports.memory.buffer) {
      return { xs: cached.xs, ys: cached.ys };
    }
    const xs = this.#column(exports, X_AXIS);
    const ys = this.#column(exports, Y_AXIS);
    this.#views = { buffer: exports.memory.buffer, xs, ys };
    return { xs, ys };
  }

  /** Releases the session. Its id is never reissued (C6), so a stale id reads
   *  {@link InvalidSessionError} rather than another session's positions. Releasing twice is
   *  refused rather than ignored: a caller surprised by a refusal has a bug, and quietly
   *  accepting it would hide one. */
  release(): void {
    this.#call("gm_force_session_release", (exports) =>
      exports.gm_force_session_release(toU32(this.#id)),
    );
    this.#views = null;
    this.#live = false;
  }

  #create(graph: Handle): ForceSessionId {
    const { exports } = this.#loaded;
    const word = invoke("gm_force_session_create", () =>
      exports.gm_force_session_create(toU32(graph), 0, 0),
    );
    if (word !== 0) return word as ForceSessionId;
    const code = lastError(exports);
    throw new ForceSessionRefusedError(`gm_force_session_create refused (${codeName(code)})`, code);
  }

  /** One position column as a `Float64Array` over the session's own storage.
   *
   *  A `Vec<f64>`'s address is 8-aligned by construction, so the `Float64Array` constructor's
   *  own alignment requirement is met. A `0` address is the motor refusing; a `0` length is a
   *  graph with no nodes, which is refused too rather than handed back as an empty column that
   *  a renderer would read as "everything is at the origin". */
  #column(exports: RawExports, axis: number): Float64Array {
    const address = invoke("gm_force_session_column_ptr", () =>
      exports.gm_force_session_column_ptr(toU32(this.#id), toU32(axis)),
    );
    const len = invoke("gm_force_session_column_len", () =>
      exports.gm_force_session_column_len(toU32(this.#id), toU32(axis)),
    );
    if (address !== 0 && len > 0) return new Float64Array(exports.memory.buffer, address, len);
    const code = lastError(exports);
    if (code === INVALID_SESSION_CODE) throw this.#dead();
    throw new ForceSessionRefusedError(`the position column (axis ${axis}) is not readable (${codeName(code)})`, code);
  }

  /** Stages `params` through `gm_alloc`, hands the address to `send`, and frees it again — on a
   *  refusal too (C7: the staging buffer is this method's job, since the caller never sees the
   *  pointer). */
  #staged(params: ForceParams, send: (exports: RawExports, ptr: number) => void): void {
    const { exports } = this.#loaded;
    const bytes = encodeParams(params);
    const ptr = invoke("gm_alloc", () => exports.gm_alloc(PARAMS_BYTES));
    if (ptr === 0) {
      throw new ForceSessionRefusedError("gm_alloc could not reserve the parameter buffer", lastError(exports));
    }
    new Uint8Array(exports.memory.buffer, ptr, PARAMS_BYTES).set(bytes);
    try {
      send(exports, ptr);
    } finally {
      invoke("gm_free", () => exports.gm_free(ptr, PARAMS_BYTES));
    }
  }

  /** Every verb's shape: call the export, turn its `0` into the typed refusal the recorded code
   *  names, and refuse to answer at all for a session this object knows is released (C6: a
   *  released id is never reissued, so every later call on it would read `InvalidSession`). */
  #call(exportName: string, call: (exports: RawExports) => number): number {
    this.#requireLive();
    const { exports } = this.#loaded;
    const word = invoke(exportName, () => call(exports));
    if (word !== 0) return word;
    const code = lastError(exports);
    if (code === INVALID_SESSION_CODE) throw this.#dead();
    throw new ForceSessionRefusedError(`${exportName} refused (${codeName(code)})`, code);
  }

  #dead(): InvalidSessionError {
    this.#live = false;
    return new InvalidSessionError(
      `force session ${this.#id} is not live (never issued, or released)`,
      INVALID_SESSION_CODE,
    );
  }

  #requireLive(): void {
    if (!this.#live) throw this.#dead();
  }
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
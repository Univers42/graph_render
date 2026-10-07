// The typed force session over `gm_force_session_*` (`docs/decisions/force-wasm-abi.md`):
// one `ForceSession` per live simulation, driven tick by tick. The physics is graph-core's
// `ForceSession` behind the wasm ABI; this file adds no arithmetic of its own — every method is
// a typed call, a `u32` coercion (C9), and a refusal turned into a `GraphMotorError` subclass
// rather than a bare `0`.
//
// Three decisions here that the wire's table records and this file is the other half of:
//
// - **Positions are `Float64Array`, not `Float32Array`.** Every other column this SDK hands
//   out is `f32`, because every other column is a *snapshot* column. A live session's columns
//   are the simulation state the next tick reads back, so narrowing them here would lose
//   precision the simulation then integrates from.
// - **The parameter defaults are read from the motor, never written here.** `params()` asks
//   `gm_force_session_params` for the session's own thirteen values, so a partial
//   `setParams` means "these fields, the motor's own value for the rest" and no copy of
//   `LiveParams::default` can go stale in this package. The wire's own half of that is
//   `force-params.ts`; the position columns' is `force-columns.ts`.
//
// **The two id spaces never share a refusal.** `InvalidSession` (15/16/17) is this file's;
// `InvalidHandle` (1) and `AllocFailed` (2) are the graph handle's and the allocator's. A
// caller debugging a dead graph is sent to the graph's code, never to a parameter range it
// cannot affect.

import { toU32 } from "./wasm.ts";
import { ForceSessionRefusedError, InvalidHandleError, InvalidSessionError } from "./errors.ts";
import { INVALID_HANDLE_CODE, frame, type Loaded } from "./calls.ts";
import { ForceColumns } from "./force-columns.ts";
import { SessionCalls } from "./force-calls.ts";
import { PARAMS_BYTES, asU32, decodeParams, mergeParams, withStagedParams } from "./force-params.ts";
import type { ForceParams, ForceSessionId, ForceTick, Handle } from "./types.ts";
import { createForceSession, type ForceStart } from "./force-create.ts";

export { PARAMS_BYTES, decodeParams, encodeParams } from "./force-params.ts";
export type { ForceStart } from "./force-create.ts";

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
  /** Every verb's refusal machinery, and the session's liveness (`force-calls.ts`). */
  readonly #calls: SessionCalls;
  /** The session's two position columns, and D9's gate in front of them (`force-columns.ts`). */
  readonly #columns: ForceColumns;

  /** @internal — use {@link Motor.forceSession}. Never throws for a load failure (the motor
   *  has already been asked, and this only reaches the ABI once it has answered): every refusal
   *  below is a typed {@link GraphMotorError}, and a refused creation leaves no session behind. */
  constructor(loaded: Loaded, graph: Handle, params?: Partial<ForceParams>, start: ForceStart = {}) {
    this.#loaded = loaded;
    this.#graph = graph;
    this.#id = createForceSession(loaded, graph, start);
    this.#calls = new SessionCalls(loaded, this.#id);
    this.#columns = new ForceColumns(loaded, this.#id);
    if (params === undefined) return;
    try {
      this.setParams(params);
    } catch (error) {
      // A refusal here would otherwise leave a session this caller never received a handle to.
      // `release()` may itself refuse; that refusal must not replace the one the caller is
      // here for, which is about their parameters and not about this cleanup. A session the
      // module would not give back stays live inside the module until it is torn down, which
      // is strictly better than losing the diagnostic.
      try {
        this.release();
      } catch {
        /* the original refusal is the one the caller must see */
      }
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
    return !this.#calls.live;
  }

  /** Runs `ticks` ticks and says what happened. **Synchronous**: the whole batch runs before
   *  this returns, which is what an interactive loop wants (one batch per frame) and what a
   *  worker wants (the frame cannot be torn in half). Bounded by `ticks` and by nothing else —
   *  the caller picks the count, so "bounded" is a statement about *when* the work finishes,
   *  never about how much of it there is.
   *
   *  `ticks` is a `u32` and is checked as one **before** it is coerced (C9). It was `toU32`'d
   *  with nothing said, so `-1` arrived as `4294967295` and ran a four-billion-tick batch
   *  synchronously inside one call — the documented promise was the opposite of what happened.
   *  Anything that is not an integer in `0..0xffffffff` is refused here, so the word that
   *  crosses the wire is the number the caller wrote.
   *
   *  `tick(112)`, 112 calls of `tick(1)` and 7 of `tick(16)` are the same positions —
   *  graph-core's guarantee, and the reason a batch settle and an interactive loop share one
   *  code path. `ticksRun` is the count that crossed the wire, not the argument's spelling.
   *
   *  Pins and parameters placed before this are what the ticks see: a verb moves nothing on its
   *  own, so a drag reads its effect on the **next** tick. */
  tick(ticks: number): ForceTick {
    // Refused before the call, and the finiteness gate before the ticks: a `NaN` written
    // through `positions()` would otherwise be integrated into every later tick, silently
    // (`sim.rs` checks nothing) and invisible to a `!==` comparison downstream.
    const count = asU32(ticks, "ticks");
    this.#calls.requireLive();
    this.#own((columns) => columns.assertFinite());
    const word = this.#calls.call("gm_force_session_tick", (exports) =>
      exports.gm_force_session_tick(toU32(this.#id), toU32(count)),
    );
    // A particle-mesh tick swaps its position columns in, so their address moves every tick.
    this.#columns.forget();
    if (word !== 1 && word !== 2) {
      throw new ForceSessionRefusedError(`gm_force_session_tick answered ${String(word)}, which is not a status`);
    }
    return { status: word === 2 ? "settled" : "running", alpha: this.#calls.alpha(), ticksRun: count };
  }

  /** The cooling schedule's current value, as the last tick left it. `0` is both a legal value
   *  and the wire's refusal value, so this resolves the ambiguity through `gm_last_error` (C4)
   *  rather than handing back a `0` a caller would read as a cold layout. */
  get alpha(): number {
    return this.#calls.alpha();
  }

  /** Holds the node at `row` at `(x, y)` exactly, from the next tick on.
   *
   *  `row` is the node's row in the two position columns — the dense order
   *  `motor.column(handle, ColumnId.NodeX)` already addresses. No node-id string crosses the
   *  ABI, so a host holding ids maps them itself (that is what the studio's drag port does).
   *  It is checked as a `u32` before it is coerced, so `NaN` is a refusal and not row 0.
   *
   *  A row past the last node, or a coordinate that is not finite, is refused by the module
   *  with the session untouched: a pin that silently did nothing would be a drag the user
   *  cannot explain. */
  pin(row: number, x: number, y: number): void {
    const at = asU32(row, "row");
    this.#calls.call("gm_force_session_pin", (exports) =>
      exports.gm_force_session_pin(toU32(this.#id), toU32(at), x, y),
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
   *  pinned is not an error: it is what "let go of this node" means when nothing held it.
   *  `row` is checked as a `u32` before it is coerced, for the reason {@link pin} is. */
  unpin(row: number): void {
    const at = asU32(row, "row");
    this.#calls.call("gm_force_session_unpin", (exports) =>
      exports.gm_force_session_unpin(toU32(this.#id), toU32(at)),
    );
  }

  /** Releases every row at once: what a host does when it stops driving the loop, so no pin
   *  survives into the next run. */
  unpinAll(): void {
    this.#calls.call("gm_force_session_unpin_all", (exports) =>
      exports.gm_force_session_unpin_all(toU32(this.#id)),
    );
  }

  /** Sets `alpha` outright — the verb behind "the user moved something, run it again". Taken
   *  exactly and **never clamped**: `0..=1`, and anything else is refused with the session left
   *  as it was, because a clamp would be a lie the caller cannot see. */
  reheat(alpha: number): void {
    this.#calls.call("gm_force_session_reheat", (exports) =>
      exports.gm_force_session_reheat(toU32(this.#id), alpha),
    );
  }

  /** Replaces some or all of the session's parameters.
   *
   *  A field the caller omits — *and a field the caller passes as `undefined`* — keeps **the
   *  motor's own current value**, read back through `gm_force_session_params` rather than from
   *  a copy of the defaults in this package, so a host can send one knob without knowing the
   *  other twelve, and cannot send the wrong twelve. Every field is range-checked by the module
   *  and never clamped; any one of them out of range refuses the whole call and changes nothing.
   *
   *  Only the per-link geometry that depends on the parameters is recomputed: positions and
   *  velocities are not reset, which is what makes this a knob and not a restart. */
  setParams(params: Partial<ForceParams>): void {
    const merged = mergeParams(this.params(), params);
    const { exports } = this.#loaded;
    withStagedParams(exports, merged, (ptr) =>
      this.#calls.call("gm_force_session_set_params", () =>
        exports.gm_force_session_set_params(toU32(this.#id), ptr, PARAMS_BYTES),
      ),
    );
  }

  /** The parameters in force, field for field, as the motor holds them. */
  params(): ForceParams {
    const ptr = this.#calls.call("gm_force_session_params", (exports) =>
      exports.gm_force_session_params(toU32(this.#id)),
    );
    return decodeParams(frame(this.#loaded.exports, ptr));
  }

  /** The two position columns, one `Float64Array` each, one entry per node in row order.
   *
   *  **Zero-copy**: both views alias the session's own columns, so this is the cheapest way to
   *  read a frame, and they are re-derived whenever wasm memory itself has been replaced since
   *  the last call (a growth detaches the old `ArrayBuffer`, which is silent garbage rather
   *  than an error) or a tick has run since: a particle-mesh tick moves the columns, so a view
   *  is re-read after every {@link ForceSession.tick}, two pointer reads.
   *
   *  **Writable, and D9 re-validates.** A session has no snapshot face, so nothing else
   *  re-checks these columns: `gm_snapshot_json`'s tamper check cannot reach them and the
   *  Rust side has no finiteness check in `sim.rs`. So this method scans both columns before
   *  handing them over, and {@link ForceSession.tick} scans them again before integrating —
   *  a `NaN` written here is refused as `TamperedGeometryError` on the next read **and** on
   *  the next tick, rather than spreading one tick further per frame. A session that has been
   *  tampered with is not repairable and says so by refusing forever.
   *
   *  Ponytail: a held view is only as fresh as the last tick. Copy it (`.slice()`) before
   *  transferring it to a worker or keeping it past the next call, exactly as this SDK's column
   *  views must be treated (C7). Failing input: a host that stores the returned arrays and
   *  reads them after driving another session — the values are live and will have moved.
   *  Direction: reading the freshest values, which is what a renderer wants. Escape hatch:
   *  call this method again per frame; it is two pointer reads and two finiteness scans. */
  positions(): { readonly xs: Float64Array; readonly ys: Float64Array } {
    this.#calls.requireLive();
    return this.#own((columns) => columns.read());
  }

  /** The two velocity columns, one `Float64Array` each, one entry per node in row order.
   *
   *  **Zero-copy and writable**, as {@link positions}: a `NaN` written through the view is
   *  refused as `TamperedGeometryError` on the next read **and** on the next tick. **Stale after
   *  a tick or a grow** — a mesh tick swaps `sim.vx`/`sim.vy` with its scratch — so copy
   *  (`.slice()`) before keeping one past the next call. */
  velocities(): { readonly vxs: Float64Array; readonly vys: Float64Array } {
    this.#calls.requireLive();
    return this.#own((columns) => columns.readVelocities());
  }

  /** Takes the session onto what {@link Motor.extend} appended to `handle`, its own graph
   *  (`gm_force_session_grow`): the same bits a fresh session carried across would hold. Every
   *  position view is stale after it. Refused, session unchanged: `InvalidSessionError`,
   *  `InvalidHandleError` for a released graph, `ForceSessionRefusedError` for another graph. */
  grow(handle: Handle): void {
    try {
      this.#calls.call("gm_force_session_grow", (e) => e.gm_force_session_grow(this.#calls.wireId, toU32(handle)));
    } catch (error) {
      if (!(error instanceof ForceSessionRefusedError) || error.code !== INVALID_HANDLE_CODE) throw error;
      throw new InvalidHandleError(`graph handle ${String(handle)} is not live`, INVALID_HANDLE_CODE);
    }
    this.#columns.forget();
  }

  /** Releases the session. Its id is never reissued (C6), so a stale id reads
   *  {@link InvalidSessionError} rather than another session's positions. Releasing twice is
   *  refused rather than ignored: a caller surprised by a refusal has a bug, and quietly
   *  accepting it would hide one. */
  release(): void {
    this.#calls.call("gm_force_session_release", (exports) =>
      exports.gm_force_session_release(toU32(this.#id)),
    );
    this.#columns.forget();
    this.#calls.close();
  }


  /** Every read of the session's own columns goes through here, so a session the module has
   *  already dropped flips this object's liveness exactly once, wherever the read happened —
   *  and only here, which is why `force-columns.ts` reports a dead session as
   *  `InvalidSessionError` and knows nothing about liveness. */
  #own<T>(read: (columns: ForceColumns) => T): T {
    try {
      return read(this.#columns);
    } catch (error) {
      if (error instanceof InvalidSessionError) throw this.#calls.dead();
      throw error;
    }
  }
}

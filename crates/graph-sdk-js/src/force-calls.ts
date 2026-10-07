// The refusal machinery every `ForceSession` verb shares: call the export, turn its `0` into
// the typed refusal the recorded code names, and refuse to answer at all for a session this
// object knows is released (C6: a released id is never reissued, so every later call on it
// would read `InvalidSession`).
//
// Its own module because it is a *policy*, not a verb: it is the one place that knows which
// codes belong to the session's id space (15 `InvalidSession`, 16 `SessionParamsInvalid`,
// 17 `SessionRefused`) and which belong to somebody else, so a verb can be written as
// "call, and do not touch the answer" without each of them re-deciding that.

import { toU32, type RawExports } from "./wasm.ts";
import { ForceSessionRefusedError, GpuMeshRefusedError, InvalidSessionError, codeName } from "./errors.ts";
import { INVALID_SESSION_CODE, invoke, lastError, type Loaded } from "./calls.ts";
import type { ForceSessionId } from "./types.ts";

/** What a session still answers while a GPU mesh drives it: its parameters, and its release. */
const UNDRIVEN: ReadonlySet<string> = new Set(["gm_force_session_params", "gm_force_session_release"]);

/** One live session's calls, whether it is still live, and whether a GPU mesh drives it. */
export class SessionCalls {
  readonly #loaded: Loaded;
  readonly #id: ForceSessionId;
  #live = true;
  /** Set while a `GpuMesh` drives the session (`force-gpu.ts`): one driver, so no verb from
   *  anyone else can move state the device holds and is about to overwrite. */
  #driven = false;
  #lifted = false;

  constructor(loaded: Loaded, id: ForceSessionId) {
    this.#loaded = loaded;
    this.#id = id;
  }

  /** Whether a call found the session already gone. Reading it does not change it; only
   *  {@link SessionCalls.requireLive} and {@link SessionCalls.dead} do. */
  get live(): boolean {
    return this.#live;
  }

  /** The session's exports, for a verb that has to read two of them to say anything. */
  get exports(): RawExports {
    return this.#loaded.exports;
  }

  /** The wire's id, already coerced: this SDK never sends a session id through `WebAssembly`
   *  uncoerced (C9). */
  get wireId(): number {
    return toU32(this.#id);
  }

  /** The cooling schedule's current value. `0` is both a legal value and the wire's refusal
   *  value, so this resolves the ambiguity through `gm_last_error` (C4) rather than handing
   *  back a `0` a caller would read as a cold layout. */
  alpha(): number {
    this.requireLive();
    const { exports } = this.#loaded;
    const alpha = invoke("gm_force_session_alpha", () => exports.gm_force_session_alpha(this.wireId));
    if (lastError(exports) === INVALID_SESSION_CODE) throw this.dead();
    return alpha;
  }

  /** Every verb's shape: call the export, return its non-zero word, and throw the refusal its
   *  recorded code names otherwise. */
  call(exportName: string, call: (exports: RawExports) => number): number {
    this.requireLive();
    if (this.#driven && !this.#lifted && !UNDRIVEN.has(exportName)) {
      throw new GpuMeshRefusedError(`${exportName}: a GPU mesh drives this session; use its verbs, or release it first`);
    }
    const word = invoke(exportName, () => call(this.#loaded.exports));
    if (word !== 0) return word;
    const code = lastError(this.#loaded.exports);
    if (code === INVALID_SESSION_CODE) throw this.dead();
    throw new ForceSessionRefusedError(`${exportName} refused (${codeName(code)})`, code);
  }

  /** Refuses before a call whose answer would be meaningless. Also the only other thing that
   *  flips {@link SessionCalls.live}, and the caller decides what a refusal means for it. */
  requireLive(): void {
    if (!this.#live) throw this.dead();
  }

  /** Marks the session gone and names it. Idempotent, and it returns the error rather than
   *  throwing so a caller can `throw this.dead()` where the message belongs. */
  dead(): InvalidSessionError {
    this.#live = false;
    return new InvalidSessionError(`force session ${String(this.#id)} is not live (never issued, or released)`, INVALID_SESSION_CODE);
  }

  get driven(): boolean {
    return this.#driven;
  }

  /** A GPU mesh takes the session (`true`) or hands it back (`false`). */
  drive(on: boolean): void {
    this.#driven = on;
  }

  /** Runs `call` as the driving mesh, past the refusal everyone else meets. Synchronous, so no
   *  other caller can run while the refusal is lifted. */
  lift<T>(call: () => T): T {
    this.#lifted = true;
    try {
      return call();
    } finally {
      this.#lifted = false;
    }
  }

  /** Marks the session gone after a release this SDK performed — the one place liveness ends
   *  without the module saying so. */
  close(): void {
    this.#live = false;
  }
}

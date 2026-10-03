// Which `gm_force_session_create*` export makes a session, and the typed refusal when the
// motor says no. Split out of `force.ts` for the house line cap.
import { toU32 } from "./wasm.ts";
import { ForceSessionRefusedError, InvalidHandleError, codeName } from "./errors.ts";
import { INVALID_HANDLE_CODE, invoke, lastError, type Loaded } from "./calls.ts";
import type { ForceEngine, ForceSeed, ForceSessionId, Handle } from "./types.ts";

/** How {@link Motor.forceSession} starts a session: its tick and where its nodes start. */
export interface ForceStart {
  readonly engine?: ForceEngine;
  readonly seed?: ForceSeed;
}

/** A new session's id. The warm export takes the engine as an argument; the spiral has one
 *  export per engine. A dead graph handle throws `InvalidHandleError`, any other refusal
 *  `ForceSessionRefusedError`; neither leaves a session behind. */
export function createForceSession(loaded: Loaded, graph: Handle, start: ForceStart): ForceSessionId {
  const { exports } = loaded;
  const mesh = start.engine === "particle_mesh" ? 1 : 0;
  const warm = start.seed === "layout";
  const spiral = mesh === 1 ? "gm_force_session_create_mesh" : "gm_force_session_create";
  const name = warm ? "gm_force_session_create_warm" : spiral;
  const word = invoke(name, () =>
    warm ? exports.gm_force_session_create_warm(toU32(graph), 0, 0, mesh) : exports[spiral](toU32(graph), 0, 0),
  );
  if (word !== 0) return word as ForceSessionId;
  const code = lastError(exports);
  if (code === INVALID_HANDLE_CODE) throw new InvalidHandleError(`graph handle ${String(graph)} is not live`, code);
  throw new ForceSessionRefusedError(`${name} refused (${codeName(code)})`, code);
}

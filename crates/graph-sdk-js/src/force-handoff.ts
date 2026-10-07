// The simple graph of a live force session: the three columns a device needs in order to run
// the same tick — each simple edge's lower endpoint, its higher endpoint, and the surviving raw
// edge's strength.
//
// Not exported from `index.ts`: this is the GPU tier's read view over the session, not part of
// the published SDK surface; `force-gpu.ts` reads it for a GPU mesh's link pass. It is a separate module
// from `force-columns.ts` for the same reason that file is separate from `force.ts`: the columns
// are a different problem from the verbs, and the one place a caller is handed a window over
// the motor's own simulation state gets one owner per kind of window.
//
// The three columns are **copied** (`.slice()`), unlike the position and velocity views: a grow
// appends edges and can move the storage, so a view would go stale silently, and a device that
// wants the graph across a grow asks again.

import { AbiContractError, ForceSessionRefusedError, InvalidSessionError, codeName } from "./errors.ts";
import { INVALID_SESSION_CODE, invoke, lastError, type Loaded } from "./calls.ts";
import { toU32, type RawExports } from "./wasm.ts";
import type { ForceSessionId } from "./types.ts";

/** The `column` argument of `gm_force_session_edge_ptr` that names the lower endpoint. */
const LO = 0;
/** …the higher endpoint. */
const HI = 1;
/** …the surviving raw edge's strength. */
const STRENGTH = 2;

/** The memory one simple-graph column lives in, once its answer is checked. A non-zero length
 *  with a zero address, an address not aligned to the element, or a range past the end of
 *  memory is a broken answer rather than a column, and is refused as `AbiContractError`. */
function checked(exports: RawExports, at: { address: number; len: number; size: number }, name: string): ArrayBuffer {
  const { address, len, size } = at;
  if (address === 0) {
    throw new AbiContractError(`${name} answered address 0 with length ${String(len)}, which is never a column's address`);
  }
  if (address % size !== 0) {
    throw new AbiContractError(`${name} answered address ${String(address)}, which is not ${String(size)}-aligned`);
  }
  const buffer = exports.memory.buffer;
  if (address + len * size > buffer.byteLength) {
    throw new AbiContractError(`${name} answered (${String(address)}, ${String(len)}), past the end of ${String(buffer.byteLength)} bytes`);
  }
  return buffer;
}

/** The session's simple graph: `(lo, hi, strength)`, one entry per simple edge in edge order.
 *
 *  `m == 0` answers three empty arrays, not a refusal: a graph whose edges are all loops or
 *  duplicates has no simple edges, and emptiness is what a device reads. A refusal reads
 *  `(0, 0)` too, so the reason (C4) is what tells them apart — `InvalidSessionError` for a
 *  session that is not live, `ForceSessionRefusedError` for anything else, the way
 *  `ForceColumns.#column` turns one. */
export function simpleEdges(
  loaded: Loaded,
  id: ForceSessionId,
): { lo: Uint32Array; hi: Uint32Array; strength: Float64Array } {
  const { exports } = loaded;
  const wire = toU32(id);
  const len = invoke("gm_force_session_edge_len", () => exports.gm_force_session_edge_len(wire));
  if (len === 0) {
    const code = lastError(exports);
    if (code === 0) {
      return { lo: new Uint32Array(0), hi: new Uint32Array(0), strength: new Float64Array(0) };
    }
    if (code === INVALID_SESSION_CODE) {
      throw new InvalidSessionError(`force session ${String(id)} is not live`, INVALID_SESSION_CODE);
    }
    throw new ForceSessionRefusedError(`the simple graph is not readable (${codeName(code)})`, code);
  }
  const ptr = (column: number): number =>
    invoke("gm_force_session_edge_ptr", () => exports.gm_force_session_edge_ptr(wire, toU32(column)));
  const words = (column: number, name: string): Uint32Array => {
    const address = ptr(column);
    return new Uint32Array(checked(exports, { address, len, size: 4 }, name), address, len).slice();
  };
  const address = ptr(STRENGTH);
  const strength = new Float64Array(checked(exports, { address, len, size: 8 }, "the strength column"), address, len);
  // Copied, not viewed: a grow can move the columns (the header).
  return { lo: words(LO, "the lo column"), hi: words(HI, "the hi column"), strength: strength.slice() };
}

/**
 * The studio's live force port over the motor's own `ForceSession`.
 *
 * Two translations, and nothing else. The SDK addresses rows, so the node's id is turned
 * into its row through the studio's own id table — the same order the snapshot and the pick
 * grid use, so row 3 is node 3 everywhere. And the four knobs become four of the motor's
 * thirteen parameters: the rest keep the motor's own values, read back through the ABI, so
 * no copy of the defaults can go stale here.
 *
 * Ponytail: `shuffle` is the layout registry's own random layout, run over the same graph
 * handle, because the ABI has no "teleport every node" verb. Failing input: a graph whose
 * node count changed since the session was made has rows the session does not have, and a
 * pin to one of them is dropped rather than refused — the drag then does nothing visible,
 * which a re-layout (which makes a new session) is the fix for. Direction: `positions()` is
 * a zero-copy view of the session's own columns, so it is only as fresh as the last tick.
 * Escape hatch: `createSession` makes a new session per layout, which re-reads the knobs.
 */
import { type ForceKnobs, type ForceParams, type ForcePort, type LiveForce } from "./live.ts";

/** Which of the motor's parameters each studio knob moves, by the wire's own field name. */
const PARAMS: readonly (readonly [keyof ForceKnobs, keyof ForceParams])[] = [
  ["gravity", "gravity"],
  ["charge", "charge"],
  ["linkStrengthScale", "link_strength_scale"],
  ["linkDistance", "link_distance"],
];

export interface MotorForceDeps<Handle> {
  /** The motor's own session over `handle`; the layout that gives the graph its positions. */
  readonly session: ForcePort;
  /** The graph the session was made over. */
  readonly handle: Handle;
  /** Node ids in the session's dense row order, or null before a layout has run. */
  readonly ids: () => readonly string[] | null;
  /** Runs the motor's random layout over `handle`, so "Animate" starts over. */
  readonly scatter: (handle: Handle) => void;
}

/**
 * The row table, built once per order. A duplicated id keeps its first row, which is what
 * `indexOf` answered before: the studio's id table has one row per node, so a duplicate is
 * a table that disagrees with itself and the first row is the one it drew.
 *
 * Caveat: keyed on the identity of the id array, so an order that is equal but a new array
 * misses and is rebuilt — the array is replaced wholesale by every layout, never edited.
 */
function rowsOf(ids: readonly string[]): Map<string, number> {
  const rows = new Map<string, number>();
  for (let row = 0; row < ids.length; row += 1) {
    const id = ids[row];
    if (id !== undefined && !rows.has(id)) rows.set(id, row);
  }
  return rows;
}

/** A `ForceParams` the four knobs can be written into, field by field. */
type Writable = { -readonly [Field in keyof ForceParams]: number };

export function createLiveForce<Handle>(deps: MotorForceDeps<Handle>): LiveForce {
  const { session, ids } = deps;
  let indexed: readonly string[] | null = null;
  let rows: Map<string, number> = new Map();
  /** One table per order, not one scan per pin: a drag move asks for a row on every frame. */
  const rowOf = (id: string): number => {
    const order = ids() ?? [];
    if (order !== indexed) {
      indexed = order;
      rows = rowsOf(order);
    }
    return rows.get(id) ?? -1;
  };
  const knobParams = (knobs: ForceKnobs): Partial<ForceParams> => {
    const out: Partial<Writable> = {};
    for (const [knob, field] of PARAMS) out[field] = knobs[knob];
    return out;
  };
  return {
    pin: (id, x, y) => {
      const row = rowOf(id);
      if (row >= 0) session.pin(row, x, y);
    },
    unpin: (id) => {
      const row = rowOf(id);
      if (row >= 0) session.unpin(row);
    },
    setParams: (knobs) => session.setParams(knobParams(knobs)),
    step: (ticks) => session.tick(ticks).alpha,
    positions: () => session.positions(),
    reheat: (alpha) => session.reheat(alpha),
    shuffle: () => {
      // The motor has no teleport verb: the random layout is the settle's own restart.
      deps.scatter(deps.handle);
      session.reheat(1);
      return 1;
    },
    params: () => session.params(),
  };
}
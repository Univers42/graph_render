/**
 * The studio's live force port over the motor's own `ForceSession`.
 *
 * Two translations, and nothing else. The SDK addresses rows, so the node's id is turned
 * into its row through the studio's own id table — the same order the snapshot and the pick
 * grid use, so row 3 is node 3 everywhere. And the nine knobs become nine of the motor's
 * thirteen parameters: the rest keep the motor's own values, read back through the ABI, so
 * no copy of their defaults can go stale here.
 *
 * Ponytail: a restart is the ABI's own release-then-create (`ForcePort.release`, then the
 * next `forceSession`), because a fresh session seeds its own start positions — so "Animate"
 * is O(n) of bookkeeping and no layout run at all. Failing input: a graph whose node count
 * changed since the session was made has rows the session does not have, and a pin to one of
 * them is dropped rather than refused — the drag then does nothing visible, which a re-layout
 * (which makes a new session) is the fix for. Direction: `positions()` is a zero-copy view of
 * the session's own columns, so it is only as fresh as the last tick. Escape hatch:
 * `createSession` makes a new session per layout run, handed the knobs this port last held.
 */
import { DEFAULT_KNOBS, type ForceKnobs, type ForceParams, type ForcePort, type LiveForce } from "./live.ts";
import type { GraphBatch } from "./protocol.ts";

/** Which of the motor's parameters each studio knob moves, by the wire's own field name. */
const PARAMS: readonly (readonly [keyof ForceKnobs, keyof ForceParams])[] = [
  ["gravity", "gravity"],
  ["charge", "charge"],
  ["linkStrengthScale", "link_strength_scale"],
  ["linkDistance", "link_distance"],
  ["collideRadius", "collide_radius"],
  ["velocityDecay", "velocity_decay"],
  ["alphaDecay", "alpha_decay"],
  ["distanceMax", "distance_max"],
  ["theta", "theta"],
];

export interface MotorForceDeps {
  /** The motor's own session over the graph; the layout that gives the graph its positions. */
  readonly session: ForcePort;
  /** Node ids in the session's dense row order, or null before a layout has run. */
  readonly ids: () => readonly string[] | null;
  /**
   * Releases the session this port is driving and answers a fresh one over the same graph and
   * engine. A new session is seeded at the motor's own start positions, so it is the restart:
   * no layout runs, and the ids order stays the one this port already holds.
   */
  readonly restart: () => ForcePort;
  /**
   * Appends a batch to the built graph. Left out by a caller with no motor behind it, and a
   * delta batch is then refused by the loop rather than applied to nothing.
   */
  readonly extend?: (batch: GraphBatch) => void;
  /** Covers the graph's new node count in the session. Left out with `extend`. */
  readonly grow?: () => void;
  /** Knobs to start with, written into the session at once; left out, the motor's own. */
  readonly knobs?: ForceKnobs;
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

/** A `ForceParams` the knobs can be written into, field by field. */
type Writable = { -readonly [Field in keyof ForceParams]: number };

/** One table per order, not one scan per pin: a drag move asks for a row on every frame. */
function rowTable(ids: () => readonly string[] | null): (id: string) => number {
  let indexed: readonly string[] | null = null;
  let rows: Map<string, number> = new Map();
  return (id) => {
    const order = ids() ?? [];
    if (order !== indexed) {
      indexed = order;
      rows = rowsOf(order);
    }
    return rows.get(id) ?? -1;
  };
}

function knobParams(knobs: ForceKnobs): Partial<ForceParams> {
  const out: Partial<Writable> = {};
  for (const [knob, field] of PARAMS) out[field] = knobs[knob];
  return out;
}

/**
 * The session the port is driving, and the knobs a restart has to write back.
 *
 * WHY the session is a binding and not the port's own field: the port keeps one identity for
 * the whole run — the loop compares ports by identity and replaces itself when one changes —
 * so only this binding moves when "Animate" restarts the settle.
 */
interface SessionState {
  /** The session in force now: the one before a restart, the fresh one after it. */
  readonly session: ForcePort;
  setParams(knobs: ForceKnobs): void;
  /** Restarts the settle and answers the alpha the new session was born at. */
  restart(): number;
  readonly knobs: ForceKnobs;
}

function sessionState(deps: MotorForceDeps): SessionState {
  let session = deps.session;
  let knobs: ForceKnobs = deps.knobs ?? DEFAULT_KNOBS;
  if (deps.knobs !== undefined) session.setParams(knobParams(deps.knobs));
  return {
    get session() {
      return session;
    },
    get knobs() {
      return knobs;
    },
    setParams: (next) => {
      // Kept, not just forwarded: a restart makes a session at the motor's own defaults, and
      // the panel's knobs are the only thing the user set that a restart must not lose.
      knobs = next;
      session.setParams(knobParams(next));
    },
    restart: () => {
      session = deps.restart();
      session.setParams(knobParams(knobs));
      return session.alpha;
    },
  };
}

export function createLiveForce(deps: MotorForceDeps): LiveForce {
  const rowOf = rowTable(deps.ids);
  const state = sessionState(deps);
  // Ponytail: the two are left out rather than stubbed, so a port over a motor with no extend
  // says so through its own absence — `force.deltas` refuses a batch it cannot extend with,
  // and a refusal that reached the page would be a lie about a graph that never changed.
  const deltas = deps.extend === undefined || deps.grow === undefined
    ? {}
    : { extend: deps.extend, grow: deps.grow };
  return {
    pin: (id, x, y) => {
      const row = rowOf(id);
      if (row >= 0) state.session.pin(row, x, y);
    },
    unpin: (id) => {
      const row = rowOf(id);
      if (row >= 0) state.session.unpin(row);
    },
    setParams: (knobs) => state.setParams(knobs),
    step: (ticks) => state.session.tick(ticks).alpha,
    positions: () => state.session.positions(),
    reheat: (alpha) => state.session.reheat(alpha),
    shuffle: () => state.restart(),
    ...deltas,
    params: () => state.session.params(),
    knobs: () => state.knobs,
  };
}

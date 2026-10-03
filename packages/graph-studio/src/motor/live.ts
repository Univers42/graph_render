/**
 * The port between the studio's worker and the motor's live force session. The physics is
 * graph-core's `ForceSession`; the studio never simulates. With no adapter the panel is
 * visible and disabled, and says why.
 */

export interface ForceKnobs {
  /** Center force: pull toward the middle, 0..1. */
  readonly gravity: number;
  /** Repel force: charge, -1000..0 (the panel shows its magnitude). */
  readonly charge: number;
  /** Link force: scale on every link's strength, 0..2. */
  readonly linkStrengthScale: number;
  /** Link distance, 10..500. */
  readonly linkDistance: number;
  /** Node spacing: the collide radius, in layout units, 0..400. */
  readonly collideRadius: number;
  /** The motor's per-tick velocity multiplier; the panel shows `1 - this` as friction. */
  readonly velocityDecay: number;
  /** Cooling: alpha's decay per tick. 0 never cools, so the panel stops at 0.005. */
  readonly alphaDecay: number;
  /** Repel range: no charge between nodes farther apart than this, in layout units. */
  readonly distanceMax: number;
  /** Accuracy: Barnes-Hut theta; lower is more exact and slower. */
  readonly theta: number;
}

export type KnobName = keyof ForceKnobs;

/** Each inside the motor's own range (`live_params.rs`), never wider. */
export const KNOB_LIMITS: Readonly<Record<KnobName, { readonly min: number; readonly max: number }>> = {
  gravity: { min: 0, max: 1 },
  charge: { min: -1000, max: 0 },
  linkStrengthScale: { min: 0, max: 2 },
  linkDistance: { min: 10, max: 500 },
  collideRadius: { min: 0, max: 400 },
  velocityDecay: { min: 0.01, max: 0.99 },
  alphaDecay: { min: 0.005, max: 0.5 },
  distanceMax: { min: 10, max: 5000 },
  theta: { min: 0.3, max: 1.5 },
};

/**
 * The motor's own defaults, so the panel shows what a fresh session runs: a new graph gets a
 * new session at these values, and a reset lands on the drawing the load made.
 *
 * Ponytail: a copy of graph-core's `ForceParams::default` (`params.rs`). A motor that moves
 * a default makes the panel lie until this follows; `live-session.motor.test.ts` reads the
 * motor's values back and fails on the first field that differs.
 */
export const DEFAULT_KNOBS: ForceKnobs = {
  gravity: 0, charge: -90, linkStrengthScale: 0.15, linkDistance: 60,
  collideRadius: 16, velocityDecay: 1 - 0.42, alphaDecay: 0.06, distanceMax: 520, theta: 0.9,
};

export const NO_ADAPTER_REASON = "live forces need the motor session (force-wasm)";

export interface LiveForce {
  /**
   * Set once the motor's session behind this port is released: every call on it throws, so
   * the stepping loop must read this before it steps. Left out by a port that never dies.
   */
  dead?: boolean;
  pin(id: string, x: number, y: number): void;
  unpin(id: string): void;
  setParams(knobs: ForceKnobs): void;
  /** Runs `ticks` steps and returns the alpha after them. */
  step(ticks: number): number;
  positions(): { readonly xs: Float64Array; readonly ys: Float64Array };
  reheat(alpha: number): void;
  /**
   * Throws the nodes back to random positions and reheats from the top. Returns the alpha it
   * left the session at, so the caller never has to guess what a restart looks like.
   */
  shuffle?(): number;
  /** The parameters the motor itself holds, by the wire's own field names. */
  params?(): ForceParams;
}

/** The wire's own parameter names, in `LiveParams`' declaration order. */
export type ForceParamField =
  | "charge" | "theta" | "distance_min" | "distance_max" | "link_distance"
  | "link_strength_scale" | "collide_radius" | "center_strength" | "gravity"
  | "velocity_decay" | "alpha_decay" | "alpha_min" | "initial_alpha";

export type ForceParams = Readonly<Record<ForceParamField, number>>;

/**
 * The motor's own live session as the studio needs it: the SDK's `ForceSession`, named by
 * the members this package calls. It addresses rows, not ids — no node-id string crosses
 * the ABI — so the adapter that turns rows into ids is `liveSession.ts`'s.
 */
export interface ForcePort {
  /** Runs `ticks` ticks; the alpha is in the answer. */
  tick(ticks: number): { readonly alpha: number };
  pin(row: number, x: number, y: number): void;
  unpin(row: number): void;
  reheat(alpha: number): void;
  /** A partial set: every field left out keeps the motor's own value for it. */
  setParams(params: Partial<ForceParams>): void;
  positions(): { readonly xs: Float64Array; readonly ys: Float64Array };
  /** Every parameter the motor holds, read back through the ABI rather than copied. */
  params(): ForceParams;
  release(): void;
}

/**
 * Ponytail: which layouts settle live is read off the id (`layout.force…`) rather than kept
 * in a table, so a force engine registered under a name nobody predicted still settles on
 * screen. Failing input: `layout.random` says nothing about force and is excluded by that
 * read, so the random layout stays a finished picture. Direction: substring, so
 * `layout.forceatlas2` and a future `layout.force.barnes_hut` both qualify.
 */
export function settlesLive(layoutId: string): boolean {
  return layoutId.startsWith("layout.force");
}

/** The tick a live session runs, by the SDK's own name for it. */
export type ForceEngine = "barnes_hut" | "particle_mesh";

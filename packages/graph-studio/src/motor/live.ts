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
}

export type KnobName = keyof ForceKnobs;

export const KNOB_LIMITS: Readonly<Record<KnobName, { readonly min: number; readonly max: number }>> = {
  gravity: { min: 0, max: 1 },
  charge: { min: -1000, max: 0 },
  linkStrengthScale: { min: 0, max: 2 },
  linkDistance: { min: 10, max: 500 },
};

export const DEFAULT_KNOBS: ForceKnobs = { gravity: 0.1, charge: -300, linkStrengthScale: 1, linkDistance: 30 };

export const NO_ADAPTER_REASON = "live forces need the motor session (force-wasm)";

export interface LiveForce {
  pin(id: string, x: number, y: number): void;
  unpin(id: string): void;
  setParams(knobs: ForceKnobs): void;
  /** Runs `ticks` steps and returns the alpha after them. */
  step(ticks: number): number;
  positions(): { readonly xs: Float64Array; readonly ys: Float64Array };
  reheat(alpha: number): void;
  /**
   * Starts the settle over from the motor's own seed and reheats from the top. Returns the
   * alpha it left the session at, so the caller never has to guess what a restart looks like.
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
  /** Moves every row to where the graph was last drawn, so a drag starts from the picture. */
  seat(): void;
  /** Moves every row back to the spiral a new session starts on, at rest. */
  restart(): void;
  /** A partial set: every field left out keeps the motor's own value for it. */
  setParams(params: Partial<ForceParams>): void;
  positions(): { readonly xs: Float64Array; readonly ys: Float64Array };
  /** Every parameter the motor holds, read back through the ABI rather than copied. */
  params(): ForceParams;
  release(): void;
}

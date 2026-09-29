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
}

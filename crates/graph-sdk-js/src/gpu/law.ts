/**
 * `law.ts` — the force parameters a rig is built at.
 *
 * A `.gmfx` carries none: the emitter ran at `ForceParams::default` (`params.rs:62-79`) with
 * `gravity = 0` (`live_params.rs:183`), and that set is `FROZEN_LAW`, written out once here
 * instead of once per stage. A live rig (`live-fixture.ts`) carries the session's own set,
 * read from the motor, so a knob the user moves reaches the device at the next rebuild.
 */

import type { ForceParams } from "../types.ts";

/** The set every `.gmfx` was emitted at, field for field. */
export const FROZEN_LAW: Readonly<ForceParams> = {
  charge: -90,
  theta: 0.9,
  distance_min: 1,
  distance_max: 520,
  link_distance: 60,
  link_strength_scale: 0.15,
  collide_radius: 16,
  center_strength: 1,
  gravity: 0,
  velocity_decay: 1 - 0.42,
  alpha_decay: 0.06,
  alpha_min: 0.001,
  initial_alpha: 1,
};

/** The set a rig over `fixture` runs at: its own, or the frozen one a `.gmfx` implies. */
export function lawOf(fixture: { readonly law?: Readonly<ForceParams> | undefined }): Readonly<ForceParams> {
  return fixture.law ?? FROZEN_LAW;
}

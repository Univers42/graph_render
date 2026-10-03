//! The live parameter set: what a session can be told, and what it will refuse.
//!
//! Every field is finite and range-checked, and a refusal is a
//! [`SessionError`](super::SessionError), never a clamp. The reasoning is in
//! `docs/decisions/live-force-session.md`; the short version is that a clamp is a lie
//! the caller cannot see, and a force layout that quietly ran with `theta = 1.5` instead of
//! the `2.0` that was asked for still returns a plausible picture.
//!
//! The ranges themselves are `ranges.rs`'s consts and nothing else — no other module states
//! a bound, so a bound can only move there, and `session/tests/m1e.rs` pins both ends of
//! every one. What is here is the struct, the two *relations* between fields that no single
//! range can express ([`relations`](Self::relations)), and the one order both validators walk.

mod ranges;

#[cfg(test)]
mod tests;

use self::ranges::{
    ALPHA_DECAY, ALPHA_MIN, CENTER_STRENGTH, CHARGE, COLLIDE_RADIUS, DISTANCE_MAX, DISTANCE_MIN,
    GRAVITY, INITIAL_ALPHA, LINK_DISTANCE, LINK_STRENGTH_SCALE, Range, THETA, VELOCITY_DECAY,
};
use super::error::SessionError;
use crate::layout::force::params::ForceParams;

pub(super) use self::ranges::{ALPHA, ALPHA_TARGET};

/// Everything a force session can be told. [`Default`] is the frozen force set
/// (`layout::force::ForceParams`), field for field, because the frozen layout *is* a
/// default session — see `docs/decisions/live-force-session.md`.
///
/// Ponytail (`gravity`): a pull toward the origin, d3's `forceX(0) + forceY(0)`, and the
/// one force the frozen set never had. Failing input: any graph whose natural extent is
/// much larger than the viewport, where nothing else brings distant structure back.
/// Direction: over-shrinking — a strong gravity collapses distinct clusters onto the
/// origin and the drawing becomes a knot, which is cosmetic rather than silent. Escape
/// hatch: it is off by default, and `0` skips the force entirely rather than applying a
/// zero strength.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiveParams {
    /// Many-body (Barnes-Hut) repulsion, the same for every node. The frozen set's
    /// `chargeStrength` (`.../layout/params.ts:28`), renamed because `charge_strength`
    /// reads as a strength and this is the signed potential term.
    pub charge: f64,
    /// Barnes-Hut opening angle; `sqrt(theta2)`, d3's own default 0.9
    /// (`manyBody.js:15`). The frozen set's Ponytail for accuracy against speed stands.
    pub theta: f64,
    /// Many-body: no force is computed between two points closer than this.
    pub distance_min: f64,
    /// Many-body: the force is cut off past this distance, a performance cutoff.
    pub distance_max: f64,
    /// Base link distance; a link's own is this over `max(0.4, edge.strength)`.
    pub link_distance: f64,
    /// Link strength scale; a link's own is `min(0.7, scale * edge.strength)`.
    pub link_strength_scale: f64,
    /// Collision radius, the same for every node.
    pub collide_radius: f64,
    /// Center-force strength: how much of the mean is subtracted per tick
    /// (`center.js:18-20`).
    pub center_strength: f64,
    /// Pull toward the origin, `d3.forceX(0).strength(this) + forceY(0)`. Zero skips the
    /// force.
    pub gravity: f64,
    /// Per-tick velocity multiplier — d3's `velocityDecay` is `1 - this`
    /// (`simulation.js:118`).
    pub velocity_decay: f64,
    /// `alpha`'s geometric decay per tick (`simulation.js:45`).
    pub alpha_decay: f64,
    /// Below this `alpha`, with no target holding it up, the run is settled.
    pub alpha_min: f64,
    /// The `alpha` a new or reheated session starts from.
    pub initial_alpha: f64,
}

impl LiveParams {
    /// Every field against its range, then every relation between two of them.
    ///
    /// The per-field half walks [`ordered`](Self::ordered) — the same list
    /// [`validate_finite`](Self::validate_finite) walks — so the two can never name a
    /// different "first bad field". [`relations`](Self::relations) runs **after** every
    /// field has passed its own range, so a value that is wrong on its own is still
    /// reported as the field that is wrong.
    pub fn validate(&self) -> Result<(), SessionError> {
        for (range, value) in self.ordered() {
            range.check(value)?;
        }
        self.relations()
    }

    /// The two cross-field invariants, each of which thirteen independent per-field ranges
    /// cannot express. Refused, like every other bad value, and for the same reason: a pair
    /// the layout cannot use is not a layout.
    ///
    /// - **`distance_min < distance_max`.** The two are read as an inner and an outer
    ///   radius of one many-body shell (`barnes_hut/charge.rs`'s `dmin2`/`dmax2`,
    ///   `particle_mesh/mesh.rs`'s `Law`), so an inside-out pair describes a shell no pair
    ///   of nodes is inside: every repulsion term is skipped and the picture comes back
    ///   plausible with no repulsion in it.
    /// - **`initial_alpha >= alpha_min`.** The first is the heat a session is *born* at and
    ///   the second is the heat below which it calls itself settled, so a session born
    ///   below its own threshold reports `settled` on tick 0, before any force has been
    ///   gathered.
    ///
    /// Ponytail (`distance_min < distance_max`, strict): the degenerate value this refuses
    /// is `distance_min == distance_max`, a shell of zero width, which is the same
    /// no-repulsion picture rather than a different one. Failing input: a caller who meant
    /// "no repulsion" and wrote it as an empty shell. Direction: over-refusing — a caller
    /// who wants no repulsion must now write `charge = 0`, which says what it means.
    /// Escape hatch: `charge = 0.0`, in range, and the honest spelling.
    fn relations(&self) -> Result<(), SessionError> {
        if self.distance_min >= self.distance_max {
            return Err(SessionError::OutOfRange {
                field: "distance_min",
                rule: "distance_min: finite, 0..=1000, and < distance_max",
            });
        }
        if self.initial_alpha < self.alpha_min {
            return Err(SessionError::OutOfRange {
                field: "initial_alpha",
                rule: "initial_alpha: finite, 0..=1, and >= alpha_min",
            });
        }
        Ok(())
    }

    /// Finiteness alone, in [`validate`](Self::validate)'s field order, so the first
    /// non-finite field is the one named. No bounds, no relations and no clamping: the
    /// frozen stage's parameter set predates the live ranges, and the only thing it refuses
    /// is a value whose bits wasm32 does not pin (D9).
    ///
    /// This is the one acceptance path that is *not* [`validate`](Self::validate), and
    /// `session/tests/frozen_path.rs` is what says so: it is reached only from
    /// [`ForceSession::from_frozen`](super::ForceSession::from_frozen), never from a live
    /// setter, so no setter can inherit its weaker promise.
    pub fn validate_finite(&self) -> Result<(), SessionError> {
        for (range, value) in self.ordered() {
            range.check_finite(value)?;
        }
        Ok(())
    }

    /// Every field against its range, in declaration order — one order for both
    /// [`validate`](Self::validate) and [`validate_finite`](Self::validate_finite), so the
    /// two can never name a different "first bad field".
    fn ordered(&self) -> [(&'static Range, f64); 13] {
        [
            (&CHARGE, self.charge),
            (&THETA, self.theta),
            (&DISTANCE_MIN, self.distance_min),
            (&DISTANCE_MAX, self.distance_max),
            (&LINK_DISTANCE, self.link_distance),
            (&LINK_STRENGTH_SCALE, self.link_strength_scale),
            (&COLLIDE_RADIUS, self.collide_radius),
            (&CENTER_STRENGTH, self.center_strength),
            (&GRAVITY, self.gravity),
            (&VELOCITY_DECAY, self.velocity_decay),
            (&ALPHA_DECAY, self.alpha_decay),
            (&ALPHA_MIN, self.alpha_min),
            (&INITIAL_ALPHA, self.initial_alpha),
        ]
    }
}

impl Default for LiveParams {
    /// The frozen force set, exactly: `ForceParams::default()` with `charge` for
    /// `charge_strength` and gravity off.
    fn default() -> Self {
        Self::from(ForceParams::default())
    }
}

impl From<ForceParams> for LiveParams {
    fn from(params: ForceParams) -> Self {
        Self {
            charge: params.charge_strength,
            theta: params.theta,
            distance_min: params.distance_min,
            distance_max: params.distance_max,
            link_distance: params.link_distance,
            link_strength_scale: params.link_strength_scale,
            collide_radius: params.collide_radius,
            center_strength: params.center_strength,
            gravity: 0.0,
            velocity_decay: params.velocity_decay,
            alpha_decay: params.alpha_decay,
            alpha_min: params.alpha_min,
            initial_alpha: params.initial_alpha,
        }
    }
}

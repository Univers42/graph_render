//! The live parameter set: what a session can be told, and what it will refuse.
//!
//! Every field is finite and range-checked, and a refusal is a
//! [`SessionError`](super::SessionError), never a clamp. The reasoning is in
//! `docs/decisions/live-force-session.md`; the short version is that a clamp is a lie
//! the caller cannot see, and a force layout that quietly ran with `theta = 1.5` instead of
//! the `2.0` that was asked for still returns a plausible picture.
//!
//! The ranges are this file's constants and nothing else — no other module states a bound,
//! so a bound can only move here, and `session/tests/m1e.rs` pins both ends of every one.

use super::error::SessionError;
use crate::layout::force::params::ForceParams;

#[cfg(test)]
mod tests;

/// One field's accepted range, and the sentence its refusal carries. `rule` is a
/// `&'static str` rather than formatted text so it can be handed straight to
/// `StageError::Param` and across the ABI without an allocation.
pub(super) struct Range {
    /// The `LiveParams` field's name.
    name: &'static str,
    /// Inclusive lower bound.
    min: f64,
    /// Inclusive upper bound.
    max: f64,
    /// The rule, with the numbers in it.
    rule: &'static str,
}

impl Range {
    /// `value` against this range: finite first, then the bounds. A NaN fails the first
    /// test and an infinity the second, which is why both are refused and neither is
    /// clamped.
    pub(super) fn check(&self, value: f64) -> Result<(), SessionError> {
        if !value.is_finite() {
            return Err(SessionError::NonFinite { field: self.name });
        }
        if value < self.min || value > self.max {
            return Err(SessionError::OutOfRange {
                field: self.name,
                rule: self.rule,
            });
        }
        Ok(())
    }

    /// `value` against this range with the upper end **open**: `min <= value < max`. One
    /// half-step's difference from [`check`](Self::check), for a field whose maximum is a
    /// value the layout cannot use.
    pub(super) fn check_open(&self, value: f64) -> Result<(), SessionError> {
        self.check_finite(value)?;
        if value < self.min || value >= self.max {
            return Err(SessionError::OutOfRange {
                field: self.name,
                rule: self.rule,
            });
        }
        Ok(())
    }

    /// `value` for finiteness only. What this range's *bounds* are is not this method's
    /// business: the frozen stage has no bounds, only the D9 rule.
    fn check_finite(&self, value: f64) -> Result<(), SessionError> {
        if value.is_finite() {
            return Ok(());
        }
        Err(SessionError::NonFinite { field: self.name })
    }
}

const CHARGE: Range = Range {
    name: "charge",
    min: -5000.0,
    max: 0.0,
    rule: "charge: finite, -5000..=0",
};
const THETA: Range = Range {
    name: "theta",
    min: 0.3,
    max: 1.5,
    rule: "theta: finite, 0.3..=1.5",
};
const DISTANCE_MIN: Range = Range {
    name: "distance_min",
    min: 0.0,
    max: 1000.0,
    rule: "distance_min: finite, 0..=1000",
};
const DISTANCE_MAX: Range = Range {
    name: "distance_max",
    min: 1.0,
    max: 1_000_000.0,
    rule: "distance_max: finite, 1..=1000000",
};
const LINK_DISTANCE: Range = Range {
    name: "link_distance",
    min: 1.0,
    max: 2000.0,
    rule: "link_distance: finite, 1..=2000",
};
const LINK_STRENGTH_SCALE: Range = Range {
    name: "link_strength_scale",
    min: 0.0,
    max: 4.0,
    rule: "link_strength_scale: finite, 0..=4",
};
const COLLIDE_RADIUS: Range = Range {
    name: "collide_radius",
    min: 0.0,
    max: 400.0,
    rule: "collide_radius: finite, 0..=400",
};
const CENTER_STRENGTH: Range = Range {
    name: "center_strength",
    min: 0.0,
    max: 1.0,
    rule: "center_strength: finite, 0..=1",
};
const GRAVITY: Range = Range {
    name: "gravity",
    min: 0.0,
    max: 1.0,
    rule: "gravity: finite, 0..=1",
};
const VELOCITY_DECAY: Range = Range {
    name: "velocity_decay",
    min: 0.01,
    max: 0.99,
    rule: "velocity_decay: finite, 0.01..=0.99",
};
const ALPHA_DECAY: Range = Range {
    name: "alpha_decay",
    min: 0.0,
    max: 1.0,
    rule: "alpha_decay: finite, 0..=1",
};
const ALPHA_MIN: Range = Range {
    name: "alpha_min",
    min: 0.0,
    max: 1.0,
    rule: "alpha_min: finite, 0..=1",
};
const INITIAL_ALPHA: Range = Range {
    name: "initial_alpha",
    min: 0.0,
    max: 1.0,
    rule: "initial_alpha: finite, 0..=1",
};
/// The cooling schedule's own value, `alpha`: `0..=1`, and both ends are values a run can
/// actually be in — a layout held at full heat, and one at rest.
pub(super) const ALPHA: Range = Range {
    name: "alpha",
    min: 0.0,
    max: 1.0,
    rule: "alpha: finite, 0..=1",
};
/// The value `alpha` moves *toward*, which is not a [`LiveParams`] field: the upper end is
/// open, because a target of exactly 1 holds the layout at full heat forever and
/// [`StepReport::settled`](super::StepReport::settled) can never be true again.
pub(super) const ALPHA_TARGET: Range = Range {
    name: "alpha_target",
    min: 0.0,
    max: 1.0,
    rule: "alpha_target: finite, 0..<1",
};

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
    /// Every field against its range, in field order, so the first bad one is named.
    pub fn validate(&self) -> Result<(), SessionError> {
        CHARGE.check(self.charge)?;
        THETA.check(self.theta)?;
        DISTANCE_MIN.check(self.distance_min)?;
        DISTANCE_MAX.check(self.distance_max)?;
        LINK_DISTANCE.check(self.link_distance)?;
        LINK_STRENGTH_SCALE.check(self.link_strength_scale)?;
        COLLIDE_RADIUS.check(self.collide_radius)?;
        CENTER_STRENGTH.check(self.center_strength)?;
        GRAVITY.check(self.gravity)?;
        VELOCITY_DECAY.check(self.velocity_decay)?;
        ALPHA_DECAY.check(self.alpha_decay)?;
        ALPHA_MIN.check(self.alpha_min)?;
        INITIAL_ALPHA.check(self.initial_alpha)
    }

    /// Finiteness alone, in [`validate`](Self::validate)'s field order, so the first
    /// non-finite field is the one named. No bounds and no clamping: the frozen stage's
    /// parameter set predates the live ranges, and the only thing it refuses is a value
    /// whose bits wasm32 does not pin (D9).
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

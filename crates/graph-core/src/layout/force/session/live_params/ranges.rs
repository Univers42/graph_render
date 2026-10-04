//! Every bound a live session enforces, in one file, as data.
//!
//! `Range` is a value rather than a function so a refusal can be handed to
//! [`StageError::Param`](crate::stage::StageError::Param) and across the ABI without an
//! allocation: the `rule` is a `&'static str` with the numbers already in it, which is why
//! these are consts and not formatted text.
//!
//! The ranges are *this* file's constants and nothing else — no other module states a
//! bound, so a bound can only move here. The two relations between fields that no single
//! range can express are `live_params.rs`'s `relations`, which is the
//! only other place a pair of these numbers is read against each other.

use crate::layout::force::session::error::SessionError;

/// One field's accepted range, and the sentence its refusal carries. `rule` is a
/// `&'static str` rather than formatted text so it can be handed straight to
/// `StageError::Param` and across the ABI without an allocation.
pub(in crate::layout::force::session) struct Range {
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
    pub(in crate::layout::force::session) fn check(&self, value: f64) -> Result<(), SessionError> {
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
    pub(in crate::layout::force::session) fn check_open(
        &self,
        value: f64,
    ) -> Result<(), SessionError> {
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
    pub(super) fn check_finite(&self, value: f64) -> Result<(), SessionError> {
        if value.is_finite() {
            return Ok(());
        }
        Err(SessionError::NonFinite { field: self.name })
    }
}

pub(super) const CHARGE: Range = Range {
    name: "charge",
    min: -5000.0,
    max: 0.0,
    rule: "charge: finite, -5000..=0",
};
pub(super) const THETA: Range = Range {
    name: "theta",
    min: 0.3,
    max: 1.5,
    rule: "theta: finite, 0.3..=1.5",
};
pub(super) const DISTANCE_MIN: Range = Range {
    name: "distance_min",
    min: 0.0,
    max: 1000.0,
    rule: "distance_min: finite, 0..=1000",
};
pub(super) const DISTANCE_MAX: Range = Range {
    name: "distance_max",
    min: 1.0,
    max: 1_000_000.0,
    rule: "distance_max: finite, 1..=1000000",
};
pub(super) const LINK_DISTANCE: Range = Range {
    name: "link_distance",
    min: 1.0,
    max: 2000.0,
    rule: "link_distance: finite, 1..=2000",
};
pub(super) const LINK_STRENGTH_SCALE: Range = Range {
    name: "link_strength_scale",
    min: 0.0,
    max: 4.0,
    rule: "link_strength_scale: finite, 0..=4",
};
pub(super) const COLLIDE_RADIUS: Range = Range {
    name: "collide_radius",
    min: 0.0,
    max: 400.0,
    rule: "collide_radius: finite, 0..=400",
};
pub(super) const CENTER_STRENGTH: Range = Range {
    name: "center_strength",
    min: 0.0,
    max: 1.0,
    rule: "center_strength: finite, 0..=1",
};
pub(super) const GRAVITY: Range = Range {
    name: "gravity",
    min: 0.0,
    max: 1.0,
    rule: "gravity: finite, 0..=1",
};
pub(super) const VELOCITY_DECAY: Range = Range {
    name: "velocity_decay",
    min: 0.01,
    max: 0.99,
    rule: "velocity_decay: finite, 0.01..=0.99",
};
pub(super) const ALPHA_DECAY: Range = Range {
    name: "alpha_decay",
    min: 0.0,
    max: 1.0,
    rule: "alpha_decay: finite, 0..=1",
};
pub(super) const ALPHA_MIN: Range = Range {
    name: "alpha_min",
    min: 0.0,
    max: 1.0,
    rule: "alpha_min: finite, 0..=1",
};
pub(super) const INITIAL_ALPHA: Range = Range {
    name: "initial_alpha",
    min: 0.0,
    max: 1.0,
    rule: "initial_alpha: finite, 0..=1",
};
/// The cooling schedule's own value, `alpha`: `0..=1`, and both ends are values a run can
/// actually be in — a layout held at full heat, and one at rest. Not a `LiveParams` field,
/// so [`ForceSession::reheat`](super::super::ForceSession::reheat) checks it here.
pub(in crate::layout::force::session) const ALPHA: Range = Range {
    name: "alpha",
    min: 0.0,
    max: 1.0,
    rule: "alpha: finite, 0..=1",
};
/// The value `alpha` moves *toward*, which is not a [`LiveParams`](super::LiveParams)
/// field: the upper end is open, because a target of exactly 1 holds the layout at full
/// heat forever and [`StepReport::settled`](super::super::StepReport::settled) can never be
/// true again.
pub(in crate::layout::force::session) const ALPHA_TARGET: Range = Range {
    name: "alpha_target",
    min: 0.0,
    max: 1.0,
    rule: "alpha_target: finite, 0..<1",
};

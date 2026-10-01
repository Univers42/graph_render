//! **M1e — parameters are refused, never clamped.** Every field is finite and range-checked,
//! and a refusal leaves the session exactly as it was. Clamping would be the alternative
//! and it is worse than useless here: a caller that asks for `theta = 2.0` and silently
//! receives `1.5` has been told a lie, and the layout that comes back looks plausible.
//!
//! The cases are generated, not written out 52 times: [`with`] is this file's own mirror of
//! `LiveParams`' fields, so a field that stopped being validated, or whose range moved,
//! fails here rather than in production.

use super::support;
use crate::layout::force::session::{LiveParams, SessionError};

/// Every field, and a value outside its range for each way a value can be wrong: below,
/// above, NaN, and each infinity.
fn refused() -> Vec<(&'static str, LiveParams, SessionError)> {
    let mut cases = Vec::new();
    for (field, min, max) in [
        ("charge", -5000.0, 0.0),
        ("theta", 0.3, 1.5),
        ("distance_min", 0.0, 1000.0),
        ("distance_max", 1.0, 1_000_000.0),
        ("link_distance", 1.0, 2000.0),
        ("link_strength_scale", 0.0, 4.0),
        ("collide_radius", 0.0, 400.0),
        ("center_strength", 0.0, 1.0),
        ("gravity", 0.0, 1.0),
        ("velocity_decay", 0.01, 0.99),
        ("alpha_decay", 0.0, 1.0),
        ("alpha_min", 0.0, 1.0),
        ("initial_alpha", 0.0, 1.0),
    ] {
        let step = (max - min) / 4.0;
        for value in [
            min - step,
            max + step,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
        ] {
            let out_of_range = value.is_finite();
            cases.push((
                field,
                with(field, value),
                if out_of_range {
                    SessionError::OutOfRange {
                        field,
                        rule: rule(field),
                    }
                } else {
                    SessionError::NonFinite { field }
                },
            ));
        }
    }
    cases
}

/// The rule a refusal carries, per field — the sentence `StageError::Param` shows.
fn rule(field: &str) -> &'static str {
    match field {
        "charge" => "charge: finite, -5000..=0",
        "theta" => "theta: finite, 0.3..=1.5",
        "distance_min" => "distance_min: finite, 0..=1000",
        "distance_max" => "distance_max: finite, 1..=1000000",
        "link_distance" => "link_distance: finite, 1..=2000",
        "link_strength_scale" => "link_strength_scale: finite, 0..=4",
        "collide_radius" => "collide_radius: finite, 0..=400",
        "center_strength" => "center_strength: finite, 0..=1",
        "gravity" => "gravity: finite, 0..=1",
        "velocity_decay" => "velocity_decay: finite, 0.01..=0.99",
        "alpha_decay" => "alpha_decay: finite, 0..=1",
        "alpha_min" => "alpha_min: finite, 0..=1",
        "initial_alpha" => "initial_alpha: finite, 0..=1",
        other => panic!("no rule for {other}"),
    }
}

/// This file's mirror of the struct: one field, one value, everything else default. The
/// `panic!` is the point — a field with no arm here is a field this file does not test.
fn with(field: &str, value: f64) -> LiveParams {
    match field {
        "charge" => LiveParams {
            charge: value,
            ..Default::default()
        },
        "theta" => LiveParams {
            theta: value,
            ..Default::default()
        },
        "distance_min" => LiveParams {
            distance_min: value,
            ..Default::default()
        },
        "distance_max" => LiveParams {
            distance_max: value,
            ..Default::default()
        },
        "link_distance" => LiveParams {
            link_distance: value,
            ..Default::default()
        },
        "link_strength_scale" => LiveParams {
            link_strength_scale: value,
            ..Default::default()
        },
        "collide_radius" => LiveParams {
            collide_radius: value,
            ..Default::default()
        },
        "center_strength" => LiveParams {
            center_strength: value,
            ..Default::default()
        },
        "gravity" => LiveParams {
            gravity: value,
            ..Default::default()
        },
        "velocity_decay" => LiveParams {
            velocity_decay: value,
            ..Default::default()
        },
        "alpha_decay" => LiveParams {
            alpha_decay: value,
            ..Default::default()
        },
        "alpha_min" => LiveParams {
            alpha_min: value,
            ..Default::default()
        },
        "initial_alpha" => LiveParams {
            initial_alpha: value,
            ..Default::default()
        },
        other => panic!("no field {other}"),
    }
}

#[test]
fn set_params_refuses_every_value_outside_its_range_and_changes_nothing() {
    for (field, params, expected) in refused() {
        let mut session = support::session(4);
        session.step(5);
        let (before, alpha) = (support::bits(&session), session.alpha());
        assert_eq!(
            session.set_params(params),
            Err(expected),
            "{field} must be refused, and by name"
        );
        assert_eq!(session.params(), LiveParams::default(), "{field}");
        assert_eq!(
            (support::bits(&session), session.alpha()),
            (before, alpha),
            "{field}"
        );
    }
}

/// The bounds are the contract, so the boundary itself is pinned: every field's minimum and
/// maximum are **accepted**, and one step past either is not. A range that quietly narrows
/// refuses a caller's legitimate value; one that widens accepts a value the layout cannot
/// use.
#[test]
fn the_ends_of_every_range_are_in_range() {
    for (field, min, max) in [
        ("charge", -5000.0, 0.0),
        ("theta", 0.3, 1.5),
        ("distance_min", 0.0, 1000.0),
        ("distance_max", 1.0, 1_000_000.0),
        ("link_distance", 1.0, 2000.0),
        ("link_strength_scale", 0.0, 4.0),
        ("collide_radius", 0.0, 400.0),
        ("center_strength", 0.0, 1.0),
        ("gravity", 0.0, 1.0),
        ("velocity_decay", 0.01, 0.99),
        ("alpha_decay", 0.0, 1.0),
        ("alpha_min", 0.0, 1.0),
        ("initial_alpha", 0.0, 1.0),
    ] {
        for value in [min, max] {
            assert!(
                with(field, value).validate().is_ok(),
                "{field} = {value} is the end of its range and must be accepted"
            );
        }
    }
}

/// The construction paths validate too: a session that cannot exist is not built, and the
/// frozen stage's parameters — the ones the 4-way hash is taken over — always are.
#[test]
fn a_session_cannot_be_built_with_parameters_it_would_refuse_later() {
    let topology = support::topology(3);
    assert_eq!(
        crate::layout::force::session::ForceSession::new(&topology, with("theta", 9.0))
            .err()
            .map(|e| e.to_string()),
        Some(
            SessionError::OutOfRange {
                field: "theta",
                rule: rule("theta")
            }
            .to_string()
        )
    );
    assert_eq!(
        crate::layout::force::session::ForceSession::new(&topology, with("charge", f64::NAN))
            .err()
            .map(|e| e.to_string()),
        Some(SessionError::NonFinite { field: "charge" }.to_string())
    );
    crate::layout::force::session::ForceSession::new(&topology, LiveParams::default())
        .expect("the defaults are in range");
}

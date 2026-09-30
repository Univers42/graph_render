//! `LiveParams`' own arithmetic: the ranges, the rules, and the fact that the default is
//! the frozen force set field for field — which is the whole premise the refactor rests on
//! (`docs/decisions/live-force-session.md`).
//!
//! The refusal of an out-of-range field, from both ends and from a non-finite value, is
//! [`m1e`](super::m1e)'s; what is here is the mapping, which nothing else in the crate
//! exercises.

use super::{LiveParams, SessionError};
use crate::layout::force::ForceParams;

#[test]
fn the_default_is_the_frozen_force_set_field_for_field() {
    assert_eq!(
        LiveParams::default(),
        LiveParams::from(ForceParams::default()),
        "the frozen stage's parameters and the session's defaults are one value"
    );
    let frozen = ForceParams::default();
    let live = LiveParams::default();
    assert_eq!(live.charge, frozen.charge_strength);
    assert_eq!(
        live.velocity_decay, frozen.velocity_decay,
        "d3's is 1 - this"
    );
    assert_eq!(live.gravity, 0.0, "off, so the frozen bytes do not move");
    assert!(LiveParams::default().validate().is_ok());
}

#[test]
fn a_conversion_keeps_every_shared_value_and_does_not_invent_one() {
    let frozen = ForceParams {
        charge_strength: -1234.5,
        theta: 1.25,
        link_distance: 7.0,
        ..ForceParams::default()
    };
    let live = LiveParams::from(frozen);
    assert_eq!(
        (live.charge, live.theta, live.link_distance),
        (-1234.5, 1.25, 7.0)
    );
    assert_eq!(live.gravity, 0.0);
}

#[test]
fn the_error_names_the_field_and_the_numbers() {
    let params = LiveParams {
        theta: 2.0,
        ..LiveParams::default()
    };
    assert_eq!(
        params.validate(),
        Err(SessionError::OutOfRange {
            field: "theta",
            rule: "theta: finite, 0.3..=1.5",
        })
    );
    assert_eq!(
        SessionError::OutOfRange {
            field: "theta",
            rule: "theta: finite, 0.3..=1.5",
        }
        .to_string(),
        "parameter theta: finite, 0.3..=1.5"
    );
    assert_eq!(
        SessionError::NonFinite { field: "xs" }.to_string(),
        "non-finite value in xs"
    );
    assert_eq!(
        SessionError::ColumnLength {
            column: "ys",
            got: 3,
            nodes: 5
        }
        .to_string(),
        "column ys: 3 values for 5 nodes"
    );
    assert_eq!(
        SessionError::NoSuchRow { row: 9, rows: 5 }.to_string(),
        "row 9 of 5"
    );
}

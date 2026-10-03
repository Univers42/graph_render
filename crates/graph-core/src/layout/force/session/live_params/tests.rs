//! `LiveParams`' own arithmetic: the ranges, the rules, and the fact that the default is
//! the frozen force set field for field — which is the whole premise the refactor rests on
//! (`docs/decisions/live-force-session.md`).
//!
//! The refusal of an out-of-range field, from both ends and from a non-finite value, is
//! [`m1e`](super::m1e)'s; what is here is the mapping, the cross-field relations, and the
//! fact that the frozen path's finiteness-only acceptance is *not* the live path's.

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

/// **LF-18.** A field can be inside its own range and the *pair* can still ask for nothing:
/// the two many-body cutoffs are read as an inner and an outer radius
/// (`barnes_hut/charge.rs`'s `dmin2`/`dmax2`, `particle_mesh/mesh.rs`'s `Law`), so a
/// `distance_min` above `distance_max` describes a shell no pair of nodes is inside — every
/// repulsion term is skipped, and the picture comes back plausible with no repulsion in it.
///
/// Per-field checks cannot see this: `1000` is the end of `distance_min`'s own range and
/// `1` is the end of `distance_max`'s, so thirteen independent `min <= v <= max` tests all
/// pass.
#[test]
fn a_distance_shell_inside_out_is_refused() {
    for (min, max) in [(1000.0, 1.0), (520.0, 519.0), (2.0, 1.0)] {
        let params = LiveParams {
            distance_min: min,
            distance_max: max,
            ..LiveParams::default()
        };
        assert_eq!(
            params.validate(),
            Err(SessionError::OutOfRange {
                field: "distance_min",
                rule: "distance_min: finite, 0..=1000, and < distance_max",
            }),
            "distance_min = {min} with distance_max = {max} is a shell no pair is inside"
        );
    }
}

/// The same finding's other end: `initial_alpha` is the heat a session is *born* at and
/// `alpha_min` is the heat below which it reports itself settled. A session born below its
/// own settle threshold is settled on tick 0 — `report()` reads `alpha < alpha_min` before
/// any force has been gathered — so a caller is told the layout finished when it has not
/// been laid out at all. `initial_alpha = 0` is inside its range; `alpha_min = 1` is inside
/// its own.
#[test]
fn a_session_born_below_its_own_settle_threshold_is_refused() {
    for (initial, min) in [(0.0, 1.0), (0.2, 0.5), (0.25, 0.5)] {
        let params = LiveParams {
            initial_alpha: initial,
            alpha_min: min,
            ..LiveParams::default()
        };
        assert_eq!(
            params.validate(),
            Err(SessionError::OutOfRange {
                field: "initial_alpha",
                rule: "initial_alpha: finite, 0..=1, and >= alpha_min",
            }),
            "initial_alpha = {initial} with alpha_min = {min} is born settled"
        );
    }
}

/// The relations are the *last* thing `validate` checks, after every field has passed its
/// own range: a value that is wrong on its own must still be reported as the field that is
/// wrong, or a caller fixing one bad field is told about the pair first.
#[test]
fn a_field_outside_its_range_is_named_before_its_pair_is() {
    let both = LiveParams {
        // `distance_min` is above `distance_max` *and* below its own minimum.
        distance_min: -1.0,
        distance_max: 1.0,
        initial_alpha: 0.0,
        alpha_min: 1.0,
        ..LiveParams::default()
    };
    assert_eq!(
        both.validate(),
        Err(SessionError::OutOfRange {
            field: "distance_min",
            rule: "distance_min: finite, 0..=1000",
        }),
        "the field's own range is checked first, in declaration order"
    );
    let pair_only = LiveParams {
        theta: 9.0,
        initial_alpha: 0.0,
        alpha_min: 1.0,
        ..LiveParams::default()
    };
    assert_eq!(
        pair_only.validate(),
        Err(SessionError::OutOfRange {
            field: "theta",
            rule: "theta: finite, 0.3..=1.5",
        }),
        "and a field that is only wrong as part of a pair waits its turn"
    );
}

/// The pairs themselves, at and either side of the line. `distance_min == distance_max` is
/// refused with the rest: it is a shell of zero width, so it is the same no-repulsion
/// picture as a shell inside out, and `charge = 0` is the honest way to ask for that.
#[test]
fn the_distance_relation_is_strict_and_the_alpha_relation_is_not() {
    let ok = |min: f64, max: f64| LiveParams {
        distance_min: min,
        distance_max: max,
        ..LiveParams::default()
    };
    assert!(ok(0.0, 1.0).validate().is_ok(), "one unit wide");
    assert!(ok(0.0, 1_000_000.0).validate().is_ok(), "the widest shell");
    assert!(
        ok(999.0, 1000.0).validate().is_ok(),
        "one unit wide, at the top of distance_min's own range"
    );
    assert!(ok(1.0, 1.0).validate().is_err(), "zero wide is refused");
    let alphas = |initial: f64, min: f64| LiveParams {
        initial_alpha: initial,
        alpha_min: min,
        ..LiveParams::default()
    };
    assert!(
        alphas(0.0, 0.0).validate().is_ok(),
        "equal is fine: settled is `alpha < alpha_min`, so alpha == alpha_min is not settled"
    );
    assert!(alphas(1.0, 0.0).validate().is_ok(), "born at full heat");
    assert!(alphas(0.001, 0.001).validate().is_ok(), "the frozen pair");
}
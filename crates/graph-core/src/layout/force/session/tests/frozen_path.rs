//! **The frozen path's acceptance is finiteness only, and no live setter can reach it.**
//!
//! `LiveParams::validate_finite` (`live_params.rs`) checks the D9 rule and nothing else: it
//! admits `theta = 2.0`, `velocity_decay = 0.0`, `link_distance = 1e9` — values the live
//! ranges refuse. That is deliberate, because the frozen stage hashes over a parameter set
//! that predates the live ranges, and re-deriving them there would refuse settings it has
//! always run. It is also exactly the promise `live_params.rs`'s header makes about the
//! other setters — *refuse, never clamp* — that a weaker check on a reachable path would
//! quietly break.
//!
//! So this file pins the shape of that weaker check: it has exactly one caller
//! ([`ForceSession::from_frozen`]), that caller's only reachable feeders are the batch arms
//! with the frozen defaults, and **every live entry point refuses what the frozen one
//! takes**. If a setter were ever routed through `validate_finite`, the second test fails:
//! it is the one that would catch a caller setting `theta = 2.0` and watching it be taken.

use super::support;
use crate::layout::force::ForceParams;
use crate::layout::force::session::{ForceSession, LiveParams, SessionError};
use crate::layout::force::params::ForceParams as Frozen;

/// The three values the review named: each inside `[0, 1]`-ish and finite, each **outside**
/// the live range, and each one the frozen stage is entitled to run.
fn only_finite_is_ok() -> LiveParams {
    LiveParams {
        theta: 2.0,
        velocity_decay: 0.0,
        link_distance: 1e9,
        ..LiveParams::default()
    }
}

#[test]
fn the_frozen_path_takes_what_the_live_ranges_refuse() {
    let params = only_finite_is_ok();
    assert!(
        params.validate_finite().is_ok(),
        "the frozen stage's acceptance is finiteness only — that is the whole point"
    );
    assert_eq!(
        params.validate(),
        Err(SessionError::OutOfRange {
            field: "theta",
            rule: "theta: finite, 0.3..=1.5",
        }),
        "and the live ranges still refuse it, by name"
    );
    let topology = support::topology(3);
    // `from_frozen` is the frozen stage's own constructor: the *frozen* set, not this one.
    let frozen = Frozen {
        theta: 2.0,
        ..ForceParams::default()
    };
    assert!(
        ForceSession::from_frozen(&topology, &frozen).is_ok(),
        "the frozen stage still runs the parameters it has always run"
    );
    assert!(
        ForceSession::new(&topology, params).is_err(),
        "and the same values cannot build a live session"
    );
}

/// The property that makes the above safe, and the one that would break first: no live
/// setter reaches the frozen acceptance. `set_params` and the warm start run the full
/// `validate`; `reheat` and `set_alpha_target` run their own single-field range. Each is
/// offered a value the frozen path would take, and each must refuse **and change nothing**.
#[test]
fn no_live_setter_inherits_the_frozen_paths_weaker_acceptance() {
    // A theta the live range refuses and the frozen acceptance takes.
    let bad = LiveParams {
        theta: 2.0,
        ..LiveParams::default()
    };
    let expected = SessionError::OutOfRange {
        field: "theta",
        rule: "theta: finite, 0.3..=1.5",
    };

    // 1. `new` — the constructor a live caller reaches first.
    let topology = support::topology(3);
    assert_eq!(ForceSession::new(&topology, bad).err(), Some(expected), "new");

    // 2. `set_params` — the mid-run setter, on a session already running.
    let mut session = support::session(4);
    session.step(5);
    let (before, alpha) = (support::bits(&session), session.alpha());
    assert_eq!(session.set_params(bad), Err(expected), "set_params");
    assert_eq!(
        (support::bits(&session), session.alpha()),
        (before, alpha),
        "a refused set_params leaves the session bit for bit as it was"
    );

    // 3. the warm start — a live construction path that is not `new`.
    let n = session.xs().len();
    let warm: Vec<f64> = (0..n).map(|i| i as f64).collect();
    assert_eq!(
        ForceSession::from_positions(&topology, bad, &warm, &warm).err(),
        Some(expected),
        "from_positions"
    );

    // 4. the two cooling setters — each has its own single-field range, and each refuses
    //    the value the frozen acceptance would take.
    assert_eq!(
        session.reheat(2.0),
        Err(SessionError::OutOfRange {
            field: "alpha",
            rule: "alpha: finite, 0..=1",
        }),
        "reheat takes 0..=1, and the frozen acceptance has no bounds to defer to"
    );
    assert_eq!(
        session.set_alpha_target(2.0),
        Err(SessionError::OutOfRange {
            field: "alpha_target",
            rule: "alpha_target: finite, 0..<1",
        }),
        "set_alpha_target takes 0..<1"
    );
}

/// The frozen stage's own parameters satisfy every live range **and** both cross-field
/// relations, so the acceptance split is a difference in strictness and not in kind: nothing
/// the frozen layout runs is something a live session would refuse.
#[test]
fn the_frozen_parameter_set_is_in_range_for_both_relations() {
    let frozen = LiveParams::from(ForceParams::default());
    assert!(
        frozen.validate().is_ok(),
        "the frozen set must satisfy the relations the frozen acceptance does not check: \
         distance_min {} >= distance_max {} would make the many-body shell empty, and \
         initial_alpha {} < alpha_min {} would be born settled",
        frozen.distance_min,
        frozen.distance_max,
        frozen.initial_alpha,
        frozen.alpha_min,
    );
}
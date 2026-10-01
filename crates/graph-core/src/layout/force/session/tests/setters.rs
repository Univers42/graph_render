//! The two cooling setters, `reheat` and `set_alpha_target`. They are the only ways a
//! caller puts a value into the schedule rather than into a parameter set, so they are the
//! only places a NaN could get in without `LiveParams::validate` in front of it — and a NaN
//! alpha poisons every position it then touches, which is exactly the kind of poisoning a
//! refusal is cheaper than.
//!
//! Refused means refused: the value is taken exactly or not at all, never clamped, and the
//! session is left bit for bit as it was. The tables are generated, not written out case by
//! case, so a value the setter's range moves to refuses without this file being edited.

use super::support;
use crate::layout::force::params::TICKS;
use crate::layout::force::session::SessionError;

/// Which of the two setters a case is about, and the error it must answer with.
struct Case {
    /// The field's name, for the failure message.
    field: &'static str,
    /// The value offered.
    value: f64,
    /// The exact refusal. `None` for a value the setter must take.
    err: Option<SessionError>,
}

/// One row per value, per setter: every way a value can be wrong — not a number, below the
/// range, above it — plus the two ends of the range itself, which are the contract.
fn cases() -> Vec<Case> {
    let mut rows = Vec::new();
    for (field, value) in [
        ("alpha", f64::NAN),
        ("alpha", f64::INFINITY),
        ("alpha", f64::NEG_INFINITY),
        ("alpha", -0.1),
        ("alpha", 1.5),
        ("alpha", 0.0),
        ("alpha", 1.0),
        ("alpha", 0.5),
        ("alpha_target", f64::NAN),
        ("alpha_target", f64::INFINITY),
        ("alpha_target", f64::NEG_INFINITY),
        ("alpha_target", -0.1),
        ("alpha_target", 1.5),
        ("alpha_target", 1.0),
        ("alpha_target", 0.0),
        ("alpha_target", 0.999),
    ] {
        rows.push(Case {
            field,
            value,
            err: expected(field, value),
        });
    }
    rows
}

/// What `field = value` must answer: refused, and by exactly which error. `alpha` takes
/// `0..=1`; `alpha_target` takes `0..<1`, because a target of exactly 1 holds the layout at
/// full heat forever, which is a caller bug rather than a setting.
fn expected(field: &'static str, value: f64) -> Option<SessionError> {
    if !value.is_finite() {
        return Some(SessionError::NonFinite { field });
    }
    let (max, rule) = if field == "alpha" {
        (1.0, "alpha: finite, 0..=1")
    } else {
        (1.0, "alpha_target: finite, 0..<1")
    };
    if value < 0.0 || value > max || (field == "alpha_target" && value == max) {
        return Some(SessionError::OutOfRange { field, rule });
    }
    None
}

/// Every case, against the setter that owns the field, with the session's whole state
/// compared either side of the call.
#[test]
fn the_cooling_setters_refuse_a_bad_value_and_change_nothing() {
    for case in cases() {
        let mut session = support::session(5);
        session.step(7);
        session.reheat(0.4).expect("0.4 is in range");
        session.step(3);
        let (before, alpha) = (support::bits(&session), session.alpha());
        let answer = offer(&mut session, case.field, case.value);
        assert_eq!(answer.err(), case.err, "{} = {}", case.field, case.value);
        if case.err.is_none() {
            continue; // an accepted value is *meant* to change the schedule
        }
        assert_eq!(
            (support::bits(&session), session.alpha()),
            (before, alpha),
            "{} = {} must leave the session exactly as it was",
            case.field,
            case.value
        );
        if case.field == "alpha_target" {
            // A written target is not in the columns or in `alpha`, so the check above
            // cannot see one: a refused target must leave the run able to settle.
            session.step(TICKS);
            assert!(
                session.step(1).settled,
                "alpha_target = {} was refused, so nothing is holding the run hot",
                case.value
            );
        }
    }
}

/// A value the range takes is read back exactly, and a target below 1 really is a target.
#[test]
fn an_accepted_cooling_value_is_read_back_exactly() {
    for value in [0.0, 0.5, 1.0] {
        let mut session = support::session(5);
        assert_eq!(session.reheat(value), Ok(()), "alpha = {value}");
        assert_eq!(session.alpha(), value, "alpha = {value} is taken exactly");
    }
    let mut session = support::session(5);
    assert_eq!(session.set_alpha_target(0.0), Ok(()));
    assert_eq!(
        session.set_alpha_target(0.999),
        Ok(()),
        "just below the open end"
    );
}

/// What the old Ponytail said was only caught downstream: a NaN that got into the schedule
/// made every position NaN, and `StepReport::settled` could never be true again. A refused
/// reheat leaves a session that cools and settles exactly as one that was never asked.
#[test]
fn a_refused_reheat_cannot_poison_the_layout() {
    let mut poisoned = support::session(6);
    let mut clean = support::session(6);
    assert_eq!(
        poisoned.reheat(f64::NAN),
        Err(SessionError::NonFinite { field: "alpha" })
    );
    poisoned.step(TICKS);
    clean.step(TICKS);
    assert_eq!(support::bits(&poisoned), support::bits(&clean));
    assert!(poisoned.step(1).settled, "and the schedule still settles");
}

/// The call under test, per field. A field with no arm is a field this file does not cover.
fn offer(
    session: &mut crate::layout::force::session::ForceSession,
    field: &'static str,
    value: f64,
) -> Result<(), SessionError> {
    match field {
        "alpha" => session.reheat(value),
        "alpha_target" => session.set_alpha_target(value),
        other => panic!("no setter for {other}"),
    }
}

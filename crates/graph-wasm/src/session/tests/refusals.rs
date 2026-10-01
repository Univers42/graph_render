//! The other half of this module's tests: the id table (C6), the error each refusal maps to,
//! and the two constants the wire carries.
//!
//! Split from `tests.rs` by the house's 300-line limit. All native (C21).

use super::super::{
    PARAMS_LEN, Status, alpha, column, create, params_of, pin, reheat, release, reset, set_params,
    tick, to_wire, unpin, unpin_all, with,
};
use super::fixture::{bits, model, params, session_over, wire_of};
use crate::errors::Code;
use graph_core::layout::force::LiveParams;

/// The two words the wire's status carries, pinned: `0` is the refusal every export returns, so
/// a settled verdict could not be `0` without colliding with it, and `1`/`2` are what an
/// interactive loop branches on.
#[test]
fn the_status_words_are_one_and_two() {
    assert_eq!(Status::Running.as_u32(), 1, "ran, still cooling");
    assert_eq!(Status::Settled.as_u32(), 2, "ran, settled");
    assert_eq!(Status::of(false), Status::Running);
    assert_eq!(Status::of(true), Status::Settled);
    assert_eq!(
        PARAMS_LEN,
        super::super::params::LEN,
        "the exported length is the buffer's own"
    );
}

#[test]
fn ids_are_issued_from_one_and_never_reused() {
    reset();
    let a = create(&model(2, 3), params()).expect("first");
    let b = create(&model(2, 3), params()).expect("second");
    assert_eq!((a, b), (1, 2), "0 stays the failure value");
    release(a).expect("released");
    assert_eq!(
        create(&model(2, 3), params()),
        Ok(3),
        "the freed id is never reissued"
    );
    assert_eq!(
        release(a),
        Err(Code::InvalidSession),
        "a released id reads as invalid, never as another session"
    );
}

#[test]
fn every_verb_refuses_a_session_that_never_existed() {
    reset();
    for id in [0, 7, u32::MAX] {
        assert_eq!(tick(id, 1), Err(Code::InvalidSession), "tick {id}");
        assert_eq!(alpha(id), Err(Code::InvalidSession), "alpha {id}");
        assert_eq!(params_of(id), Err(Code::InvalidSession), "params {id}");
        assert_eq!(
            set_params(id, params()),
            Err(Code::InvalidSession),
            "set_params {id}"
        );
        assert_eq!(reheat(id, 0.5), Err(Code::InvalidSession), "reheat {id}");
        assert_eq!(pin(id, 0, 1.0, 2.0), Err(Code::InvalidSession), "pin {id}");
        assert_eq!(unpin(id, 0), Err(Code::InvalidSession), "unpin {id}");
        assert_eq!(unpin_all(id), Err(Code::InvalidSession), "unpin_all {id}");
        assert_eq!(column(id, 0, true), Err(Code::InvalidSession), "ptr {id}");
        assert_eq!(column(id, 1, false), Err(Code::InvalidSession), "len {id}");
        assert_eq!(release(id), Err(Code::InvalidSession), "release {id}");
    }
}

/// A refusal leaves the session exactly as it was — the property graph-core states for every
/// setter, and the reason this module holds no copy of the last accepted parameters.
#[test]
fn a_refused_verb_leaves_the_session_exactly_as_it_was() {
    let id = session_over(16);
    tick(id, 10).expect("runs");
    let before = wire_of(id, 0);

    assert_eq!(
        pin(id, 999, 0.0, 0.0),
        Err(Code::SessionRefused),
        "past the last row"
    );
    assert_eq!(
        pin(id, 0, f64::NAN, 0.0),
        Err(Code::SessionRefused),
        "not finite (D9)"
    );
    assert_eq!(
        set_params(
            id,
            LiveParams {
                theta: 9.0,
                ..params()
            }
        ),
        Err(Code::SessionRefused),
        "out of range, never clamped"
    );
    assert_eq!(reheat(id, 1.5), Err(Code::SessionRefused), "outside 0..=1");
    assert_eq!(
        params_of(id),
        Ok(params()),
        "the refused set changed nothing"
    );
    assert_eq!(bits(&wire_of(id, 0)), bits(&before), "and moved nothing");
}

/// Out of range at creation is refused too, so a session never exists with parameters its own
/// setters would reject — and a refused creation issues no id, which is what keeps a caller
/// that counted on getting one from holding a session it never saw.
#[test]
fn a_session_is_never_created_with_parameters_it_would_refuse() {
    reset();
    for bad in [
        LiveParams {
            theta: 0.1,
            ..params()
        },
        LiveParams {
            charge: 1.0,
            ..params()
        },
        LiveParams {
            gravity: f64::INFINITY,
            ..params()
        },
    ] {
        assert_eq!(
            create(&model(2, 4), bad),
            Err(Code::SessionRefused),
            "{bad:?}"
        );
    }
    assert_eq!(
        tick(create(&model(2, 4), params()).expect("only a valid one"), 1),
        Ok(Status::Running),
        "and the first valid creation is the first id"
    );
}

/// The column addresses are the session's own storage, and an address the wire cannot carry is
/// refused rather than truncated: a `u32` cut out of the middle of a 64-bit heap address is a
/// wild pointer in JavaScript, and the refusal is a named code the host reads back (C4).
///
/// The address's *stability* is not testable here — on a 64-bit host it is never reportable —
/// so it is stated where it is true: `Sim`'s columns are never resized (the only writer that
/// could is `ForceSession::set_positions`, behind `from_positions`, which this ABI does not
/// export), and the session table holds each session behind a `Box` so an insert cannot move it.
/// `graph-cli force-gate`'s wasm arm reads these columns through the wire's `(ptr, len)` on every
/// seed, which is where a moved address would show up.
#[test]
fn an_address_the_wire_cannot_carry_is_refused_rather_than_truncated() {
    let id = session_over(10);
    for axis in [0, 1] {
        assert_eq!(column(id, axis, false), Ok(10), "axis {axis} length");
        let reported = column(id, axis, true);
        let real = with(id, |session| {
            Ok(if axis == 0 {
                session.xs().as_ptr() as usize
            } else {
                session.ys().as_ptr() as usize
            })
        })
        .expect("a live session");
        match u32::try_from(real) {
            Ok(fits) => assert_eq!(
                reported,
                Ok(fits),
                "axis {axis}: a fitting address, exactly"
            ),
            Err(_) => assert_eq!(
                reported,
                Err(Code::IndexOutOfRange),
                "axis {axis}: refused, never a truncated address"
            ),
        }
    }
    assert_ne!(
        with(id, |session| Ok(session.xs().as_ptr() as usize)).expect("live"),
        with(id, |session| Ok(session.ys().as_ptr() as usize)).expect("live"),
        "the two columns are separate allocations"
    );
}

/// The conversion behind that refusal, on its own: exactly what fits crosses unchanged, and one
/// past the wire's widest word is refused with [`Code::IndexOutOfRange`] — never truncated,
/// which on a 64-bit host would turn a heap address into a wild pointer.
#[test]
fn to_wire_carries_exactly_what_fits_and_refuses_the_rest() {
    assert_eq!(to_wire(0), Ok(0));
    assert_eq!(to_wire(u32::MAX as usize), Ok(u32::MAX));
    assert_eq!(
        to_wire(u32::MAX as usize + 1),
        Err(Code::IndexOutOfRange),
        "one past the wire's widest word"
    );
    assert_eq!(to_wire(usize::MAX), Err(Code::IndexOutOfRange));
}

/// An axis the ABI does not name is refused by name, not read as `x`.
#[test]
fn an_axis_outside_the_two_named_ones_is_refused() {
    let id = session_over(4);
    assert_eq!(column(id, 2, true), Err(Code::IndexOutOfRange));
    assert_eq!(column(id, u32::MAX, false), Err(Code::IndexOutOfRange));
    assert_eq!(column(id, 0, false), Ok(4));
    assert_eq!(column(id, 1, false), Ok(4));
}

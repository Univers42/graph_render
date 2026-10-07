//! The velocity and edge columns' own cases: that the lengths and addresses the two calls
//! answer are the session's own columns and not a copy that could drift, and that an axis or
//! column the ABI does not name — or a session that is not live — reads `0` with the reason
//! recorded (C4).
//!
//! Split from `tests.rs` by the house's 300-line limit. All native (C21): a 64-bit host cannot
//! follow a heap address through the wire's `u32`, so the address half is checked against the
//! session's own pointer and the *values* are read through `with` — `fixture::wire_of`'s own
//! reason. The wasm32 half, following the address itself, is `graph-cli force-gate`.

use super::super::super::exports::session_handoff::{
    gm_force_session_edge_len, gm_force_session_edge_ptr, gm_force_session_velocity_len,
    gm_force_session_velocity_ptr,
};
use super::super::handoff::{edge, velocity};
use super::fixture::{bits, model, params, session_over};
use crate::errors::{self, Code};
use graph_core::layout::force::ForceSession;

/// One velocity column as the session holds it, read through `with` — the native stand-in for
/// following the address the export answered.
fn wire_velocity(id: u32, axis: u32) -> Vec<f64> {
    super::super::with(id, |session| {
        Ok(match axis {
            0 => session.vxs().to_vec(),
            _ => session.vys().to_vec(),
        })
    })
    .expect("a live session")
}

/// The simple graph as the session holds it, read through `with`.
fn wire_edges(id: u32) -> (Vec<u32>, Vec<u32>, Vec<f64>) {
    super::super::with(id, |session| {
        let (lo, hi, strength) = session.simple_edges();
        Ok((lo.to_vec(), hi.to_vec(), strength.to_vec()))
    })
    .expect("a live session")
}

/// The address the export answered against the session's own column's address: equal when the
/// wire's `u32` can carry it, [`Code::IndexOutOfRange`] when it cannot — never a truncated
/// address, which on a 64-bit host would be a wild pointer in JavaScript.
fn assert_address_is_the_sessions_own(reported: Result<u32, Code>, real: usize, what: &str) {
    match u32::try_from(real) {
        Ok(fits) => assert_eq!(reported, Ok(fits), "{what}: a fitting address, exactly"),
        Err(_) => assert_eq!(
            reported,
            Err(Code::IndexOutOfRange),
            "{what}: refused, never a truncated address"
        ),
    }
}

/// The velocity columns are the session's own: one per node, and after a tick the values
/// behind the answered addresses are `ForceSession::vxs()`/`vys()` bit for bit.
#[test]
fn the_velocity_columns_are_the_sessions_own() {
    let mut direct = ForceSession::new(&model(3, 24), params()).expect("in range");
    direct.step(50);

    let id = session_over(24);
    for axis in [0, 1] {
        assert_eq!(velocity(id, axis, false), Ok(24), "axis {axis} length");
    }
    super::super::tick(id, 50).expect("runs");
    for axis in [0, 1] {
        let real = super::super::with(id, |session| {
            Ok(match axis {
                0 => session.vxs().as_ptr() as usize,
                _ => session.vys().as_ptr() as usize,
            })
        })
        .expect("a live session");
        assert_address_is_the_sessions_own(velocity(id, axis, true), real, "the velocity column");
    }
    assert_eq!(
        bits(&wire_velocity(id, 0)),
        bits(direct.vxs()),
        "the vx column"
    );
    assert_eq!(
        bits(&wire_velocity(id, 1)),
        bits(direct.vys()),
        "the vy column"
    );
}

/// The three edge columns are the simple graph: the same `m` for all three, and the values
/// behind the answered addresses are `ForceSession::simple_edges()`'s own.
#[test]
fn the_edge_columns_are_the_simple_graph() {
    let mut direct = ForceSession::new(&model(3, 24), params()).expect("in range");
    direct.step(50);

    let id = session_over(24);
    let (lo, hi, strength) = wire_edges(id);
    assert_eq!(lo.len(), hi.len(), "one endpoint per edge");
    assert_eq!(lo.len(), strength.len(), "one strength per edge");
    assert!(lo.len() > 1, "the model has edges");
    for column in [0, 1, 2] {
        assert_eq!(
            edge(id, column, false),
            Ok(lo.len() as u32),
            "column {column} length"
        );
    }
    for (column, real) in [
        (0, lo.as_ptr() as usize),
        (1, hi.as_ptr() as usize),
        (2, strength.as_ptr() as usize),
    ] {
        assert_address_is_the_sessions_own(edge(id, column, true), real, "the edge column");
    }
    assert_eq!(lo.as_slice(), direct.simple_edges().0, "lo");
    assert_eq!(hi.as_slice(), direct.simple_edges().1, "hi");
    assert_eq!(strength.as_slice(), direct.simple_edges().2, "strength");
}

/// An axis or column the ABI does not name is refused by name through the exports, not read as
/// `vx` or `lo` — the same refusal `refusals.rs` pins for the position columns.
#[test]
fn a_bad_axis_or_column_reads_zero() {
    let id = session_over(4);
    for axis in [2, 3, u32::MAX] {
        assert_eq!(
            (gm_force_session_velocity_ptr(id, axis), errors::get()),
            (0, Code::IndexOutOfRange as u32),
            "velocity ptr, axis {axis}"
        );
        assert_eq!(
            (gm_force_session_velocity_len(id, axis), errors::get()),
            (0, Code::IndexOutOfRange as u32),
            "velocity len, axis {axis}"
        );
    }
    // `gm_force_session_edge_len` takes no column argument — it answers `edge(session, 0,
    // false)` — so only the `ptr` half can meet a column the ABI does not name.
    for column in [3, 4, u32::MAX] {
        assert_eq!(
            (gm_force_session_edge_ptr(id, column), errors::get()),
            (0, Code::IndexOutOfRange as u32),
            "edge ptr, column {column}"
        );
    }
    // The named ones still answer, and a success clears the reason (C4).
    assert_eq!(gm_force_session_velocity_len(id, 0), 4);
    assert_eq!(
        errors::get(),
        Code::None as u32,
        "success clears the reason"
    );
    assert_eq!(gm_force_session_velocity_len(id, 1), 4);
    let m = wire_edges(id).0.len() as u32;
    assert!(m > 1, "the model has edges");
    assert_eq!(gm_force_session_edge_len(id), m);
    assert_eq!(
        errors::get(),
        Code::None as u32,
        "success clears the reason"
    );
}

/// A session that is not live reads `0` with `InvalidSession` — never another session's
/// columns, which is what the id table's never-reissued ids (C6) are there to prevent.
#[test]
fn a_dead_session_reads_zero() {
    for id in [0, 7, u32::MAX] {
        assert_eq!(
            (gm_force_session_velocity_ptr(id, 0), errors::get()),
            (0, Code::InvalidSession as u32),
            "velocity ptr, id {id}"
        );
        assert_eq!(
            (gm_force_session_velocity_len(id, 1), errors::get()),
            (0, Code::InvalidSession as u32),
            "velocity len, id {id}"
        );
        assert_eq!(
            (gm_force_session_edge_ptr(id, 0), errors::get()),
            (0, Code::InvalidSession as u32),
            "edge ptr, id {id}"
        );
        assert_eq!(
            (gm_force_session_edge_len(id), errors::get()),
            (0, Code::InvalidSession as u32),
            "edge len, id {id}"
        );
    }
}

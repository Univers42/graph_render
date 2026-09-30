//! **M1d — a pin is exact, and a row is the node's own row.** d3 pins a node with `fx`/`fy`
//! and its own tick tail honours them (`simulation.js:53-56`): `x = fx`, `vx = 0`. A pin
//! that is *nearly* exact is worse than no pin, because a dragged node that springs back is
//! the visible symptom.
//!
//! The second half is the addressing promise: [`NodeRow`] is a node's row in the snapshot
//! columns, and today that row is its dense index. The dense index never leaves the motor
//! (`prompt.md` §4), so the *mapping* is all that can be pinned — and it is, over every node
//! of a topology, so it cannot drift quietly.

use super::support;
use crate::layout::force::session::{ForceSession, LiveParams, NodeRow};

#[test]
fn a_pinned_node_sits_exactly_at_its_pin_after_every_single_step() {
    let topology = support::topology(5);
    let (fx, fy) = (0.0, 0.0);
    let mut session =
        ForceSession::new(&topology, LiveParams::default()).expect("the defaults are in range");
    session.pin(NodeRow::new(0), fx, fy).expect("row 0 exists");
    for tick in 1..=20 {
        session.step(1);
        assert_eq!(
            (session.xs()[0].to_bits(), session.ys()[0].to_bits()),
            (fx.to_bits(), fy.to_bits()),
            "tick {tick}: a pinned node is placed, not moved"
        );
    }
}

/// Row `i` is node `i`'s own column, for every node — pinned at four distinct places, so a
/// mapping that were off by one, reversed or scaled fails on the first row it gets wrong.
#[test]
fn a_row_is_the_nodes_own_row_in_the_columns() {
    let topology = support::topology(5);
    for row in 0..topology.node_count() {
        let (fx, fy) = (f64::from(row) * 100.0 + 0.5, f64::from(row) * -7.0 - 0.25);
        let mut session =
            ForceSession::new(&topology, LiveParams::default()).expect("the defaults are valid");
        session.pin(NodeRow::new(row), fx, fy).expect("in range");
        session.step(8);
        assert_eq!(
            (
                session.xs()[row as usize].to_bits(),
                session.ys()[row as usize].to_bits()
            ),
            (fx.to_bits(), fy.to_bits()),
            "row {row} is node {row}'s own column"
        );
        let elsewhere: Vec<u32> = (0..topology.node_count())
            .filter(|&other| other != row)
            .filter(|&other| session.xs()[other as usize] == fx)
            .collect();
        assert!(
            elsewhere.is_empty(),
            "row {row}: no other column is at the pin, but {elsewhere:?} is"
        );
    }
}

/// Unpinning hands the node back to the forces: its velocity is zero at the pin, so it
/// starts again from rest and the layout moves on without it. A pin that outlives
/// `unpin` is the failure a user sees as a node stuck in the corner.
#[test]
fn an_unpinned_node_moves_again() {
    let mut session = support::session(5);
    let (fx, fy) = (0.0, 0.0);
    session.pin(NodeRow::new(0), fx, fy).expect("row 0 exists");
    session.step(8);
    assert_eq!(session.xs()[0], fx);
    session.unpin(NodeRow::new(0)).expect("row 0 exists");
    let before = session.xs()[0];
    session.step(8);
    assert_ne!(
        session.xs()[0],
        before,
        "the pin is gone and the forces have it again"
    );
}

/// `unpin_all` is the difference between "release the node under the cursor" and "release
/// everything", and it must not depend on the order pins were set in.
#[test]
fn unpin_all_releases_every_node_at_once() {
    let topology = support::topology(5);
    let mut all = ForceSession::new(&topology, LiveParams::default()).expect("defaults are valid");
    for row in 0..topology.node_count() {
        all.pin(NodeRow::new(row), 0.0, 0.0).expect("in range");
    }
    all.step(4);
    assert!(
        all.xs().iter().all(|&x| x == 0.0),
        "every node is on its pin"
    );
    all.unpin_all();
    all.step(4);
    assert!(
        all.xs().iter().any(|&x| x != 0.0),
        "with every pin released the layout is free again"
    );
}

/// Pins outside the topology are refused, and refusing one leaves the session alone: a
/// silent no-op here is a drag that does nothing and no error to explain it.
#[test]
fn a_pin_outside_the_topology_is_refused_and_changes_nothing() {
    let topology = support::topology(3);
    let rows = topology.node_count();
    let mut session =
        ForceSession::new(&topology, LiveParams::default()).expect("the defaults are valid");
    let before = support::bits(&session);
    for row in [rows, rows + 1, u32::MAX] {
        assert_eq!(
            session.pin(NodeRow::new(row), 0.0, 0.0),
            Err(crate::layout::force::session::SessionError::NoSuchRow { row, rows }),
            "row {row} of {rows}"
        );
        assert_eq!(
            session.unpin(NodeRow::new(row)),
            Err(crate::layout::force::session::SessionError::NoSuchRow { row, rows })
        );
    }
    assert_eq!(support::bits(&session), before);
}

/// A pin at a coordinate wasm32 does not pin a bit pattern for is refused the same way
/// (D9) — and *not* clamped, because a pin quietly moved to the nearest finite value is a
/// node in a place the caller did not ask for.
#[test]
fn a_pin_at_a_non_finite_coordinate_is_refused() {
    use crate::layout::force::SessionError;
    let mut session = support::session(3);
    for x in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            session.pin(NodeRow::new(0), x, 0.0),
            Err(SessionError::NonFinite { field: "pin.x" }),
            "x = {x}"
        );
    }
    for y in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            session.pin(NodeRow::new(0), 0.0, y),
            Err(SessionError::NonFinite { field: "pin.y" }),
            "y = {y}"
        );
    }
    session.step(1);
    assert!(
        session
            .xs()
            .iter()
            .chain(session.ys())
            .all(|v| v.is_finite()),
        "and nothing non-finite was let in"
    );
}

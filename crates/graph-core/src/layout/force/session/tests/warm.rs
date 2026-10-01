//! The warm start: a session that begins from positions the caller already has. It is the
//! verb that makes a live session worth having — nudge a picture, continue a stored one,
//! hand a force layout the output of another layout — and it is the only constructor whose
//! input is data rather than a topology.

use super::support;
use crate::layout::force::session::{ForceSession, LiveParams, SessionError};

/// The warm start is the same session with its positions replaced: a caller that already
/// has a picture (a previous run, a stored snapshot, another layout's output) gets a
/// simulation that continues from it rather than from the golden spiral.
#[test]
fn a_warm_start_continues_from_the_positions_it_is_given() {
    let mut cold = support::session(6);
    let (xs, ys) = (cold.xs().to_vec(), cold.ys().to_vec());
    let mut warm =
        ForceSession::from_positions(&support::topology(6), LiveParams::default(), &xs, &ys)
            .expect("the positions are the ones this crate just produced");
    assert_eq!((warm.xs(), warm.ys()), (xs.as_slice(), ys.as_slice()));
    cold.step(20);
    warm.step(20);
    assert_eq!(support::bits(&cold), support::bits(&warm));
    assert_ne!(
        warm.xs(),
        xs.as_slice(),
        "and it is a continuation, not a copy"
    );
}

/// A warm start from somewhere else is a different picture: the seed is only a starting
/// point, not a fixed point of the layout.
#[test]
fn a_warm_start_moves_away_from_where_it_started() {
    let topology = support::topology(6);
    let coincident = vec![0.0; topology.node_count() as usize];
    let mut warm =
        ForceSession::from_positions(&topology, LiveParams::default(), &coincident, &coincident)
            .expect("finite");
    warm.step(1);
    assert!(
        warm.xs().iter().any(|&x| x != 0.0),
        "an exact coincidence is separated by the counter hash, not left to divide by zero"
    );
}

/// A warm start is refused when it does not describe this topology, or when it holds a
/// value whose bits wasm32 does not pin (D9). Both leave nothing behind.
#[test]
fn a_warm_start_that_does_not_fit_is_refused() {
    let topology = support::topology(3);
    let nodes = topology.node_count();
    let zeros = vec![0.0; nodes as usize];
    for (xs, ys, expected) in [
        (
            vec![0.0; nodes as usize + 1],
            zeros.clone(),
            SessionError::ColumnLength {
                column: "xs",
                got: u64::from(nodes) + 1,
                nodes,
            },
        ),
        (
            zeros.clone(),
            vec![0.0; nodes as usize - 1],
            SessionError::ColumnLength {
                column: "ys",
                got: u64::from(nodes) - 1,
                nodes,
            },
        ),
        (
            vec![f64::NAN; nodes as usize],
            zeros.clone(),
            SessionError::NonFinite { field: "xs" },
        ),
        (
            zeros.clone(),
            vec![f64::INFINITY; nodes as usize],
            SessionError::NonFinite { field: "ys" },
        ),
    ] {
        assert_eq!(
            ForceSession::from_positions(&topology, LiveParams::default(), &xs, &ys).err(),
            Some(expected)
        );
    }
}

/// A restart is a new session over the same graph: back on the spiral, at rest, at tick 0 and
/// the starting alpha, so the same ticks give the same bytes — whatever ran before it. Pins
/// are the host's and survive it.
#[test]
fn a_restart_replays_a_new_session_bit_for_bit() {
    let mut fresh = support::session(6);
    let mut used = support::session(6);
    used.step(37);
    used.restart();
    assert_eq!(
        support::bits(&used),
        support::bits(&fresh),
        "back on the spiral"
    );
    assert_eq!(used.alpha(), fresh.alpha());
    fresh.step(25);
    used.step(25);
    assert_eq!(
        support::bits(&used),
        support::bits(&fresh),
        "and it settles the same way"
    );
}

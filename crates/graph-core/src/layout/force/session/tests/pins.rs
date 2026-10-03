//! **The pin verbs: place a row, then hand it back.** The same contract `verbs.rs`
//! establishes — nothing moves at the verb, one tick later the rows are the forces' again —
//! read against `sim.rs`'s integrate tail rather than against a parameter, because a pin is
//! the one verb that writes a *position* rather than arming a force.
//!
//! The pin's own accuracy over many ticks is `m1d.rs`'s, and a refused pin (a row past the
//! last column, a non-finite coordinate) is `pin.rs`'s own tests. What is here is the
//! timing: a pin is a value the tick consumes, not a move the verb performs, and a release
//! whose failure mode is a node stuck in a corner for ever.

use super::support::{PX, PY, ROW, SEED, Twin, WARMUP};
use crate::layout::force::session::{ForceSession, NodeRow};

/// Which release verb a case is about: `unpin` frees one row, `unpin_all` frees every
/// row. The same contract with a different blast radius, so both are worth one case.
#[derive(Clone, Copy)]
enum Release {
    /// [`ForceSession::unpin`], on [`ROW`] alone.
    Row,
    /// [`ForceSession::unpin_all`].
    Everything,
}

/// A pin is consumed at the *tail* of the next tick, in `integrate`, which places the row
/// exactly and zeroes its velocity. So the row's own bits are the pin's bits — a nearly
/// exact pin is a node that springs back under the cursor, which `m1d.rs` watches over
/// many ticks and this watches for the tick the verb was given on.
#[test]
fn a_pin_changes_the_next_tick_and_nothing_before_it() {
    let mut twin = Twin::twinned(SEED);
    twin.step(WARMUP);
    twin.assert_same("two runs of one model are the same bytes before any verb");
    twin.subject
        .pin(NodeRow::new(ROW), PX, PY)
        .expect("row 0 exists and both coordinates are finite");
    twin.assert_same("a pin is not a move: the row is still where the forces left it");
    twin.step(1);
    twin.assert_differ("and one tick later it has been placed, not integrated");
    assert_eq!(
        (
            twin.subject.xs()[ROW as usize].to_bits(),
            twin.subject.ys()[ROW as usize].to_bits()
        ),
        (PX.to_bits(), PY.to_bits()),
        "a pinned row is placed at the pin, bit for bit"
    );
}

/// Releasing is the verb whose failure is a node stuck in the corner forever, and it is
/// the one with no argument a caller can get wrong: `unpin` frees the row it is given,
/// `unpin_all` frees every row. Both are the pin contract run backwards — nothing moves
/// at the verb, and the row is handed back to the forces by the next tick's tail.
#[test]
fn releasing_a_pin_changes_the_next_tick_and_nothing_before_it() {
    for release in [Release::Row, Release::Everything] {
        let mut twin = Twin::twinned(SEED);
        for run in [&mut twin.reference, &mut twin.subject] {
            run.pin(NodeRow::new(ROW), PX, PY).expect("row 0 exists");
        }
        twin.step(WARMUP);
        twin.assert_same("both runs are pinned the same way, so both are the same bytes");
        release_from(&mut twin.subject, release);
        twin.assert_same("a release is not a move: nothing has been integrated yet");
        twin.step(1);
        twin.assert_differ("and one tick later the released rows are the forces' again");
        match release {
            Release::Row => {
                assert_eq!(
                    twin.reference.xs()[ROW as usize].to_bits(),
                    PX.to_bits(),
                    "the reference is still exactly on its pin"
                );
                assert_ne!(
                    twin.subject.xs()[ROW as usize].to_bits(),
                    PX.to_bits(),
                    "and the subject's row is the forces' again"
                );
            }
            Release::Everything => assert!(
                twin.subject
                    .xs()
                    .iter()
                    .any(|&x| x.to_bits() != PX.to_bits()),
                "every row was released, so none of them is still sitting on a pin"
            ),
        }
    }
}

/// One release verb on the subject, `force::Session` having refused nothing here.
fn release_from(session: &mut ForceSession, release: Release) {
    let freed = match release {
        Release::Row => session.unpin(NodeRow::new(ROW)),
        Release::Everything => {
            session.unpin_all();
            Ok(())
        }
    };
    freed.expect("row 0 exists");
}

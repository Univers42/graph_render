//! **An accepted verb is not an edit, it is an arming.** The contract this file
//! establishes, once for each of the six forces the spec names — repulsion, link
//! distance, gravity, centre, pin, drag:
//!
//! 1. two runs of the same model, stepped the same number of ticks, are the same bytes;
//! 2. the verb goes to one of them and **the positions do not move** — there is no queue
//!    and no deferred apply, so what a verb has not done is write a column;
//! 3. one tick later the two runs are apart.
//!
//! Step 2 is the half that reads as obvious and is not. A verb that recomputed the
//! layout to answer its caller would make every interactive step a synchronous relayout,
//! and `m1b`'s "the tick count is the only thing that changes the bytes" would be false
//! for any caller who touched the session in between — which is every UI that offers a
//! drag. Steps 1 and 3 together are what say a force is genuinely in the next tick's
//! deltas rather than in a field nobody reads: a parameter that was stored and never
//! gathered would pass a read-back test (`setters.rs`) and fail this one.
//!
//! `setters.rs` is the other half — a **refused** verb changes nothing at all, and an
//! accepted one changes exactly the next tick. Together the two files are the whole
//! setter contract: take the value exactly, or leave the session alone.

use super::support;
use crate::layout::force::session::{ForceSession, LiveParams, NodeRow};

/// The model both runs of every case here share: 7 nodes, so every force has a
/// neighbour to push against and a pair far enough apart to push.
const SEED: u32 = 5;

/// How many ticks both runs are stepped *before* the verb, so the forces have real
/// velocities rather than the zero row the spiral seed starts from — a verb applied to
/// an unmoved layout proves less than one applied to a running one.
const WARMUP: u32 = 12;

/// The row `pin` and the release case address, and a pin far outside anything the layout
/// reaches on its own, so a pinned row cannot land where it would have landed anyway.
const ROW: u32 = 0;
const PX: f64 = -250.0;
const PY: f64 = 175.0;

/// Which release verb a case is about: `unpin` frees one row, `unpin_all` frees every
/// row. The same contract with a different blast radius, so both are worth one case.
#[derive(Clone, Copy)]
enum Release {
    /// [`ForceSession::unpin`], on [`ROW`] alone.
    Row,
    /// [`ForceSession::unpin_all`].
    Everything,
}

/// Two runs of the same model, stepped in lockstep, one of which will be told something
/// the other is not. The whole file is the distance between them.
struct Twin {
    /// The run nothing is ever said to: what the subject is measured against.
    reference: ForceSession,
    /// The run every verb in this file goes to.
    subject: ForceSession,
}

impl Twin {
    /// Two fresh sessions over the gate's model for `seed`. A pair that differed by one
    /// field would make every assertion here compare two different problems.
    fn twinned(seed: u32) -> Self {
        let topology = support::topology(seed);
        let new = || ForceSession::new(&topology, LiveParams::default()).expect("in range");
        Self {
            reference: new(),
            subject: new(),
        }
    }

    /// Steps both runs the same number of ticks — the only thing that may differ between
    /// two runs of one model, and what `m1b` pins.
    fn step(&mut self, ticks: u32) {
        self.reference.step(ticks);
        self.subject.step(ticks);
    }

    /// The two runs are the same bytes, `when` naming the point in the contract.
    fn assert_same(&self, when: &str) {
        assert_eq!(
            support::bits(&self.subject),
            support::bits(&self.reference),
            "{when}"
        );
    }

    /// The two runs are not the same bytes, `when` naming what was supposed to move.
    fn assert_differ(&self, when: &str) {
        assert_ne!(
            support::bits(&self.subject),
            support::bits(&self.reference),
            "{when}"
        );
    }
}

/// Steps `diverge`'s steps 1–2 for a changed parameter: both runs warm, both identical,
/// the verb on the subject alone, and the positions **still** identical because a verb
/// only ever arms the next tick. Returns the pair one tick later, for the caller to say
/// what its force did.
fn diverge(seed: u32, force: &str, params: LiveParams) -> Twin {
    let mut twin = Twin::twinned(seed);
    twin.step(WARMUP);
    twin.assert_same("two runs of one model are the same bytes before any verb");
    twin.subject
        .set_params(params)
        .unwrap_or_else(|_| panic!("{force} is in range"));
    twin.assert_same(&format!(
        "{force}: set_params has moved nothing, it has armed the next tick"
    ));
    twin.step(1);
    twin
}

/// Repulsion is the many-body charge, and it is the one force whose sign convention is
/// worth a test: it is stored as a negative potential term, so the "changed" value here is
/// a *stronger negative*. A run that did not move would mean the charge column was read
/// back but never gathered, and every graph would quietly lose its only repulsion while
/// still looking plausible — which is exactly what a zero charge looks like.
#[test]
fn repulsion_changes_the_next_tick_and_nothing_before_it() {
    let mut twin = diverge(
        SEED,
        "charge",
        LiveParams {
            charge: -300.0,
            ..LiveParams::default()
        },
    );
    twin.assert_differ("the charge the subject was given is in the deltas it gathered");
    twin.step(1);
    twin.assert_differ("and it is not a one-tick wobble that the next tick erases");
}

/// Link distance is the one parameter that is *not* read per node: `set_params` rebuilds
/// the per-link `distance`/`strength`/`bias` columns immediately, so this also pins the
/// claim that rebuilding that geometry touches no position — a geometry rebuild that also
/// nudged a column would move the layout at the verb, not at the tick.
#[test]
fn link_distance_changes_the_next_tick_and_nothing_before_it() {
    let twin = diverge(
        SEED,
        "link_distance",
        LiveParams {
            link_distance: 200.0,
            ..LiveParams::default()
        },
    );
    twin.assert_differ("the rebuilt per-link geometry is what the link pass gathers from");
}

/// Gravity is the force the frozen set never had, and the only one that is *skipped*
/// rather than applied at zero (`sim.rs`'s guard, `session/gravity.rs` is the
/// arithmetic). So the observable change here is off → on, and the test would fail for
/// the wrong reason if the value only proved that a zero-strength force is not a no-op.
#[test]
fn gravity_changes_the_next_tick_and_nothing_before_it() {
    let twin = diverge(
        SEED,
        "gravity",
        LiveParams {
            gravity: 0.1,
            ..LiveParams::default()
        },
    );
    twin.assert_differ("a force that was skipped is now gathered, and the layout knows it");
}

/// Centre is a translation of the mean, not a force: it carries no alpha factor and moves
/// position rather than velocity (`center.js:18-20`). Turning it *off* is therefore the
/// direction that shows it was there — a session with the force on is re-centred every
/// tick and one with it off keeps the drift the other forces gave it.
#[test]
fn centre_changes_the_next_tick_and_nothing_before_it() {
    let mut twin = diverge(
        SEED,
        "center_strength",
        LiveParams {
            center_strength: 0.0,
            ..LiveParams::default()
        },
    );
    twin.assert_differ("the mean is no longer subtracted from either column");
    twin.step(1);
    twin.assert_differ("and the drift it would have cancelled is still not cancelled");
}

/// Drag is `velocity_decay`, the per-tick multiplier in the integrate tail
/// (`simulation.js:55`) — the one force whose whole effect is on a column the caller
/// cannot read. `0.58` is the frozen value and `0.01` the range's other end, so the
/// subject's next tick is almost pure displacement with no momentum carried over.
#[test]
fn drag_changes_the_next_tick_and_nothing_before_it() {
    let twin = diverge(
        SEED,
        "velocity_decay",
        LiveParams {
            velocity_decay: 0.01,
            ..LiveParams::default()
        },
    );
    twin.assert_differ("the velocity the subject integrates with is nearly killed");
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

/// The verb behind "the user moved something, run it again": `alpha` is read at the top
/// of the next tick, so the same "nothing now, everything next" contract holds for the
/// schedule itself. `live.rs` owns the exact cooling arithmetic and the one-tick
/// `alpha` read-back; what only this file says is that the heat reaches *positions*.
#[test]
fn a_reheat_changes_the_next_tick_and_nothing_before_it() {
    let mut twin = Twin::twinned(SEED);
    twin.step(WARMUP);
    twin.assert_same("two runs of one model are the same bytes before any verb");
    twin.subject.reheat(1.0).expect("1.0 is in range");
    twin.assert_same("a reheat is not a move: the columns are where the last tick left them");
    twin.step(1);
    twin.assert_differ("and the next tick scales every force by the heat it was given");
}

/// `alpha_target` is the same lever one step further out: it changes what `alpha` moves
/// *toward*, so it is consumed by the same top-of-tick line. `live.rs` watches the long
/// run — a target holding 1000 ticks hot and then releasing — and this only asks whether
/// one tick of a held-hot schedule reaches the positions.
#[test]
fn an_alpha_target_changes_the_next_tick_and_nothing_before_it() {
    let mut twin = Twin::twinned(SEED);
    twin.step(WARMUP);
    twin.assert_same("two runs of one model are the same bytes before any verb");
    twin.subject
        .set_alpha_target(0.3)
        .expect("0.3 is below the open end");
    twin.assert_same("a target is not a move: nothing has been integrated yet");
    twin.step(1);
    twin.assert_differ("and the next tick cools toward the target rather than toward zero");
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

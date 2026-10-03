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
//!
//! The pin verbs are `pins.rs`'s: they place a row rather than gathering a force, and they
//! are read against `sim.rs`'s integrate tail rather than against a parameter.

use super::support::{SEED, WARMUP, Twin, diverge};
use crate::layout::force::session::LiveParams;

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

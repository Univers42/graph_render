//! **M1b — chunking invariance.** `step(112)`, `112 × step(1)` and `7 × step(16)` are the
//! same simulation, so they are the same bytes. This is the property that lets a caller
//! drive an interactive loop (one tick a frame) and a batch settle (all 112 at once) from
//! one code path — and the only thing that catches a tick that reads the wall clock, the
//! tick counter, or anything else outside its own arguments.

use super::support;

#[test]
fn the_tick_count_is_the_only_thing_that_depends_on_how_the_ticks_are_chunked() {
    let whole = stepped(112);
    let singles = chunked(1, 112);
    let sixteens = chunked(16, 7);
    assert_eq!(support::bits(&whole), support::bits(&singles), "112 x 1");
    assert_eq!(support::bits(&whole), support::bits(&sixteens), "7 x 16");
}

fn stepped(ticks: u32) -> crate::layout::force::session::ForceSession {
    let mut session = support::session(11);
    session.step(ticks);
    session
}

fn chunked(each: u32, times: u32) -> crate::layout::force::session::ForceSession {
    let mut session = support::session(11);
    for _ in 0..times {
        let report = session.step(each);
        assert_eq!(
            (report.ticks_run, report.alpha),
            (each, session.alpha()),
            "the report says what the chunk did"
        );
    }
    session
}

/// Zero ticks is a step that changes nothing and says so — the boundary a caller driving
/// "step until settled" hits on the iteration where the predicate is already true.
#[test]
fn stepping_zero_times_is_a_step_that_moves_nothing() {
    let mut session = support::session(11);
    let before = support::bits(&session);
    let report = session.step(0);
    assert_eq!((report.ticks_run, report.alpha), (0, session.alpha()));
    assert_eq!(support::bits(&session), before);
}

/// A run of chunks stops as soon as one reports itself settled, and the picture it stops
/// at is the picture the same total of ticks reaches in one call.
#[test]
fn a_caller_can_step_until_settled_and_land_on_the_same_bytes() {
    let mut until_settled = support::session(11);
    let mut ticks = 0;
    loop {
        let report = until_settled.step(1);
        ticks += 1;
        if report.settled {
            break;
        }
        assert!(
            ticks <= 512,
            "alpha must reach alpha_min: alpha_target is 0 here"
        );
    }
    let mut at_once = support::session(11);
    at_once.step(ticks);
    assert_eq!(support::bits(&until_settled), support::bits(&at_once));
    assert_eq!(
        ticks, 112,
        "the least k with 0.94^k < 0.001 is 112 (prompt.md §5.2)"
    );
}

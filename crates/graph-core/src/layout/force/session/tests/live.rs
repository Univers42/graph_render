//! Everything else the session's API does, once per branch: the alpha schedule, gravity, the
//! settle predicate, the buffer reuse the tick loop is required to have, and the degenerate
//! topologies. The warm start has its own file (`warm.rs`) and the five named determinism
//! cases have theirs.

use super::support;
use crate::index::Topology;
use crate::layout::force::session::{ForceSession, LiveParams, NodeRow};

/// `reheat` is `simulation.js`'s own `alpha(_)` setter: the value is taken exactly, and the
/// next tick decays it from there. It is the verb behind "the user moved something, run it
/// again", and it is the reason a live session is not 112 ticks and then over.
#[test]
fn reheat_sets_alpha_exactly_and_the_next_tick_cools_it() {
    let mut session = support::session(2);
    session.step(112);
    assert!(session.alpha() < 0.001, "cold: below alpha_min");
    session.reheat(0.5).expect("0.5 is in range");
    assert_eq!(session.alpha(), 0.5, "reheat is exact, not a nudge");
    let report = session.step(1);
    assert_eq!(
        report.alpha,
        0.5 + (0.0 - 0.5) * 0.06,
        "then one tick of cooling"
    );
}

/// `alpha_target` is what makes a session *live* rather than a countdown: a non-zero target
/// holds alpha up, so the layout keeps responding, and the settle predicate refuses to call
/// that settled.
#[test]
fn a_non_zero_alpha_target_holds_the_layout_hot_and_never_settles() {
    let mut session = support::session(2);
    session
        .set_alpha_target(0.3)
        .expect("0.3 is below the open end");
    for _ in 0..1000 {
        assert!(!session.step(1).settled, "a held-hot layout is not settled");
    }
    assert!(
        (session.alpha() - 0.3).abs() < 1e-3,
        "alpha converges on the target, it does not climb past it: {}",
        session.alpha()
    );
    session.set_alpha_target(0.0).expect("0 is in range");
    let mut ticks = 0;
    while !session.step(1).settled {
        ticks += 1;
        assert!(ticks <= 4096, "releasing the target must cool it again");
    }
}

/// The cooling schedule is d3's own line, `alpha += (alphaTarget - alpha) * alphaDecay`
/// (`simulation.js:45`) — and at `alphaTarget = 0` it is bit for bit the simpler
/// `alpha += -alpha * alphaDecay` the frozen layout used to write, for every alpha a
/// cooling schedule reaches. Asserted tick by tick rather than argued, so the d3 form is
/// what runs and the equivalence is a fact rather than a claim.
#[test]
fn the_cooling_schedule_is_d3s_form_and_agrees_with_its_simplification() {
    let mut session = support::session(2);
    let mut alpha = LiveParams::default().initial_alpha;
    for tick in 1..=112 {
        session.step(1);
        alpha = alpha + (-alpha) * 0.06;
        assert_eq!(session.alpha().to_bits(), alpha.to_bits(), "tick {tick}");
    }
}

/// A target *below* `alpha_min` is the case that separates this motor's settle predicate
/// from d3's own `alpha < alphaMin` (`simulation.js:33`): alpha really does fall below the
/// threshold, and the layout really is still being driven, so it is not settled.
#[test]
fn a_target_below_alpha_min_is_still_held_hot() {
    let mut session = support::session(2);
    session
        .set_alpha_target(0.0001)
        .expect("0.0001 is in range");
    for _ in 0..2000 {
        assert!(!session.step(1).settled, "a held-hot layout is not settled");
    }
    assert!(
        session.alpha() < session.params().alpha_min,
        "alpha {} has fallen below alpha_min, and the target is holding it there",
        session.alpha()
    );
}

/// Settled is `alpha < alpha_min` **and** `alpha_target == 0` — stricter than d3's own
/// `alpha < alphaMin` (`simulation.js:33`) on purpose: a layout being deliberately held hot
/// is not a layout that has stopped moving.
#[test]
fn settled_needs_both_a_cold_alpha_and_no_target() {
    let mut session = support::session(2);
    session.step(1);
    assert!(!session.step(1).settled, "alpha is still above alpha_min");
    session.step(110);
    assert!(session.alpha() < 0.001);
    assert!(session.step(1).settled);
}

/// Gravity is d3's `forceX(0) + forceY(0)` (`x.js:13`) — `vx += (0 - x) * strength * alpha`,
/// in that order, left to right, and nothing else. The topology is two unconnected nodes
/// with every other force at zero, so the one tick below *is* the formula and its bit
/// pattern is the assertion.
#[test]
fn gravity_is_force_x_and_force_y_at_the_origin() {
    let (strength, decay) = (0.5, 0.99);
    let quiet = LiveParams {
        charge: 0.0,
        collide_radius: 0.0,
        center_strength: 0.0,
        velocity_decay: decay,
        gravity: strength,
        ..LiveParams::default()
    };
    let (xs, ys) = (vec![40.0, -40.0], vec![0.0, 0.0]);
    let mut session = ForceSession::from_positions(&two_loose_nodes(), quiet, &xs, &ys)
        .expect("the parameters are in range");
    session.step(1);
    // alpha decays first (simulation.js:45), so the tick acts on 1 - alpha_decay.
    let alpha = 1.0 + (0.0 - 1.0) * 0.06;
    let velocity = |x: f64| (0.0 - x) * strength * alpha * decay;
    for (i, &x) in xs.iter().enumerate() {
        assert_eq!(
            session.xs()[i].to_bits(),
            (x + velocity(x)).to_bits(),
            "node {i}: (0 - x) * strength * alpha, then velocity decay, then integrate"
        );
        assert_eq!(session.ys()[i].to_bits(), 0.0f64.to_bits(), "no y to pull");
    }
}

/// Two nodes and no edges, so link, charge and collide have nothing to do and the only
/// force in the tick is the one under test.
fn two_loose_nodes() -> Topology {
    let nodes = [
        crate::records::build::node("a", ""),
        crate::records::build::node("b", ""),
    ];
    crate::index::index_model(&nodes, &[]).expect("fits")
}

/// The default is the frozen layout's own bytes, and the tick **skips** the force at zero
/// strength rather than applying it: `(0 - x) * 0.0` is `+0.0` for every `x < 0`, and
/// `-0.0 + +0.0` is `+0.0`, a different `f64` with different bytes. `session/gravity.rs`
/// has the one-line proof, and `sim/tests.rs` has the reachability note — through the tick
/// no velocity can be `-0.0`, so this test pins that the default is *off* rather than that
/// the skip is load-bearing today.
#[test]
fn gravity_at_zero_is_the_frozen_layout_and_is_skipped_not_applied() {
    let mut zero = ForceSession::new(&support::topology(9), LiveParams::default())
        .expect("the defaults are valid");
    let mut explicit =
        ForceSession::new(&support::topology(9), gravity(0.0)).expect("0 is in range");
    zero.step(40);
    explicit.step(40);
    assert_eq!(
        support::bits(&zero),
        support::bits(&explicit),
        "gravity 0 is the default and the frozen stage's bytes depend on it"
    );
    assert_eq!(
        LiveParams::default().gravity,
        0.0,
        "and the default is off, not 0.0001"
    );
}

fn gravity(strength: f64) -> LiveParams {
    LiveParams {
        gravity: strength,
        ..LiveParams::default()
    }
}

/// The tick loop allocates nothing per tick: the quadtree and the scratch buffers are
/// cleared and refilled, so their capacity is at steady state after the first tick and
/// stays there. A session that grew its buffers as the graph moved would make a settle
/// cost more than the settle before it.
#[test]
fn the_scratch_buffers_are_reused_across_ticks() {
    let mut session = support::session(21);
    session.step(150);
    let warm = session.scratch_capacities();
    session.step(200);
    assert_eq!(
        session.scratch_capacities(),
        warm,
        "200 more ticks at a settled layout must not have grown a single buffer"
    );
}

/// A pin is a promise about one node, and `set_params` must not break it.
#[test]
fn changing_the_parameters_keeps_the_pins_where_they_are() {
    let mut session = support::session(8);
    session
        .pin(NodeRow::new(1), 3.0, -4.0)
        .expect("row 1 exists");
    session.step(3);
    session
        .set_params(LiveParams {
            charge: -300.0,
            ..LiveParams::default()
        })
        .expect("-300 is in range");
    session.step(3);
    assert_eq!(
        (session.xs()[1].to_bits(), session.ys()[1].to_bits()),
        (3.0f64.to_bits(), (-4.0f64).to_bits())
    );
}

/// An empty topology is a session with no columns, and stepping it is not an error — the
/// frozen layout already runs it (`barnes_hut/tests.rs`).
#[test]
fn an_empty_topology_is_a_session_that_steps_to_nothing() {
    let empty = crate::index::empty_model();
    let mut session =
        ForceSession::new(&empty, LiveParams::default()).expect("the defaults are in range");
    let report = session.step(3);
    assert_eq!(report.ticks_run, 3);
    assert!(!report.settled, "three ticks is nowhere near alpha_min");
    assert!(session.xs().is_empty() && session.ys().is_empty());
    assert!(
        session.step(112).settled,
        "an empty run still cools to alpha_min"
    );
}

/// The node count the session addresses against, which is what a row is refused against.
#[test]
fn a_rows_are_the_snapshot_columns() {
    let topology: Topology = support::topology(0);
    assert_eq!(topology.node_count(), 2);
    let session = support::session(0);
    assert_eq!(session.xs().len(), topology.node_count() as usize);
    assert_eq!(session.ys().len(), topology.node_count() as usize);
}

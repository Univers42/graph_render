//! The load-bearing half of this module's tests: **N ticks driven through these functions are
//! the same bits as `ForceSession::step(N)` called directly.** Without it, "one tick, one pin,
//! one column" is a claim about the code as written and not about the code as shipped — and a
//! second, subtly different tick loop in this layer would pass every other test here.
//!
//! Split from `tests.rs` by the house's 300-line limit. All native (C21): no wasm build in the
//! loop, and the wasm32 half of the same claim is `graph-cli force-gate`.

use super::super::{Engine, Status, alpha, create, params_of, pin, reheat, tick, unpin};
use super::fixture::{bits, model, params, session_over, wire_of};
use graph_core::layout::force::{ForceSession, NodeRow};

/// A new session carries the defaults and starts at `initial_alpha` — before any tick, and with
/// nothing to say it is settled.
#[test]
fn a_new_session_is_the_default_parameters_and_a_cold_start() {
    let id = session_over(6);
    assert_eq!(params_of(id), Ok(params()), "the defaults are in force");
    assert_eq!(alpha(id), Ok(1.0), "initial_alpha, before any tick");
    assert_eq!(
        tick(id, 0),
        Ok(Status::Running),
        "no ticks, not settled yet"
    );
}

/// **The determinism claim.** N ticks through this module's functions against
/// `ForceSession::step(N)` on a session built the same way, compared as bits.
///
/// 1, 2 and 50 ticks: the first is a single tick's own rounding, the second the chunking the
/// session promises is irrelevant (`step(n)` is `n × step(1)`), and 50 is the count
/// `graph-cli force-gate` drives — the same number, both of its arms.
#[test]
fn the_wasm_facing_functions_reach_the_same_bits_as_step() {
    for ticks in [1, 2, 50] {
        let mut direct = ForceSession::new(&model(3, 24), params()).expect("in range");
        let report = direct.step(ticks);

        let id = session_over(24);
        assert_eq!(
            tick(id, ticks),
            Ok(Status::of(report.settled)),
            "{ticks} ticks"
        );
        assert_eq!(
            alpha(id).expect("live").to_bits(),
            report.alpha.to_bits(),
            "alpha after {ticks} ticks"
        );
        assert_eq!(bits(&wire_of(id, 0)), bits(direct.xs()), "the x column");
        assert_eq!(bits(&wire_of(id, 1)), bits(direct.ys()), "the y column");
        assert_eq!(wire_of(id, 0).len(), 24, "one row per node");
    }
}

/// The determinism claim for a particle-mesh session: ticks through this module against
/// `with_particle_mesh().step(N)`. The column is re-read after every call, because the mesh
/// tick moves it.
#[test]
fn a_mesh_session_reaches_the_same_bits_as_step() {
    let topology = model(3, 24);
    let mut direct = ForceSession::new(&topology, params())
        .expect("in range")
        .with_particle_mesh();
    let report = direct.step(50);

    let id = create(&topology, params(), Engine::ParticleMesh).expect("in range");
    for _ in 0..5 {
        tick(id, 10).expect("runs");
    }
    assert_eq!(
        alpha(id).expect("live").to_bits(),
        report.alpha.to_bits(),
        "alpha"
    );
    assert_eq!(bits(&wire_of(id, 0)), bits(direct.xs()), "the x column");
    assert_eq!(bits(&wire_of(id, 1)), bits(direct.ys()), "the y column");
}

/// The same claim for the *verbs*, not only the ticks: a session pinned, released and reheated
/// through this module lands where one driven through `ForceSession` lands. Without this a
/// second, wrong pin rule could hide behind the tick test above.
#[test]
fn the_pins_and_reheat_reach_the_same_bits_as_the_session_methods() {
    let mut direct = ForceSession::new(&model(3, 12), params()).expect("in range");
    direct.pin(NodeRow::new(3), 40.0, -25.0).expect("in range");
    direct.unpin(NodeRow::new(5)).expect("in range");
    direct.reheat(0.5).expect("in range");
    direct.step(30);

    let id = session_over(12);
    pin(id, 3, 40.0, -25.0).expect("in range");
    unpin(id, 5).expect("in range");
    reheat(id, 0.5).expect("in range");
    tick(id, 30).expect("runs");
    assert_eq!(
        alpha(id).expect("live").to_bits(),
        direct.alpha().to_bits(),
        "alpha"
    );
    assert_eq!(bits(&wire_of(id, 0)), bits(direct.xs()), "the x column");
    assert_eq!(bits(&wire_of(id, 1)), bits(direct.ys()), "the y column");
}

/// Pin places a node and it holds there while the rest of the layout moves — the whole point of
/// a drag, and what the SDK smoke case watches for. Pins move nothing until the next tick
/// (`docs/decisions/live-force-session.md`), so this ticks before reading.
#[test]
fn a_pinned_node_holds_its_place_while_its_neighbour_moves() {
    let id = session_over(16);
    pin(id, 0, 100.0, -100.0).expect("in range");
    tick(id, 20).expect("runs");
    let xs = wire_of(id, 0);
    assert_eq!(xs[0].to_bits(), 100.0f64.to_bits(), "the pin held exactly");
    assert!(
        xs[1..].iter().any(|x| *x != 0.0),
        "the rest of the layout moved off the golden spiral at all"
    );
    assert!(
        xs[1..].iter().all(|x| *x != 100.0),
        "and only the pinned row is at the pin's x"
    );

    unpin(id, 0).expect("in range");
    tick(id, 40).expect("runs");
    assert_ne!(
        wire_of(id, 0)[0].to_bits(),
        100.0f64.to_bits(),
        "released, it integrates again"
    );
}

/// A settled verdict is the one an interactive loop stops on, so it has to be reachable from
/// the wire's status word and not only from `StepReport` — and a reheat has to be able to take
/// it back out, which is the drag-again case.
#[test]
fn a_long_run_settles_and_a_reheat_makes_it_run_again() {
    let id = session_over(16);
    assert_eq!(tick(id, 400), Ok(Status::Settled), "cooled past alpha_min");
    assert!(
        alpha(id).expect("live") < params().alpha_min,
        "settled means cooled past alpha_min"
    );
    reheat(id, 1.0).expect("in range");
    assert_eq!(
        tick(id, 0),
        Ok(Status::Running),
        "hot again, so not settled"
    );
}

/// Two sessions over the same topology are two independent simulations, not one shared state:
/// the ids are separate and ticking one must not move the other.
#[test]
fn two_sessions_over_one_graph_are_independent() {
    let topology = model(4, 16);
    let first = create(&topology, params(), Engine::BarnesHut).expect("first");
    let second = create(&topology, params(), Engine::BarnesHut).expect("second");
    tick(first, 20).expect("runs");
    assert_eq!(
        wire_of(second, 0),
        {
            let untouched = create(&topology, params(), Engine::BarnesHut).expect("third");
            wire_of(untouched, 0)
        },
        "the untouched session sits on the same seed spiral"
    );
    assert_ne!(
        wire_of(first, 0),
        wire_of(second, 0),
        "the ticked one moved"
    );
}

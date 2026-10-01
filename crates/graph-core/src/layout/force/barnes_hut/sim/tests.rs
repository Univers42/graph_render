//! The tick's own guards, on states a caller cannot reach through the session's API but a
//! force kernel can produce: the ones that decide bytes.
//!
//! The frozen layout's 65 goldens (`session/tests/m1a.rs`) are the outer guarantee; these
//! are the inner ones, each isolating one `if` in [`Sim::tick`] so that removing it is a
//! failing test rather than a reviewer's guess.

use super::{How, Sim};
use crate::exec::Serial;
use crate::index::index_model;
use crate::layout::force::{ForceParams, LiveParams, Split};
use crate::records::build::node;

/// One serial tick — the same `Sim::tick` the session and the frozen stage run, over the
/// serial `How`, so a test here and a session step are the same computation.
fn tick(sim: &mut Sim) {
    let mut deltas = Vec::new();
    let mut how = How {
        runner: &Serial,
        workers: 1,
        deltas: &mut deltas,
        split: Split::None,
    };
    sim.tick(&mut how);
}

fn two_nodes() -> Sim {
    let nodes = [node("a", ""), node("b", "")];
    let topology = index_model(&nodes, &[]).expect("fits");
    Sim::new(&topology, LiveParams::from(ForceParams::default()), 0)
}

/// The same two nodes with every force but the one under test switched off, so a tick's
/// only effect is the line in question.
fn quiet(gravity: f64) -> Sim {
    let mut sim = two_nodes();
    sim.params.gravity = gravity;
    sim.params.charge = 0.0;
    sim.params.collide_radius = 0.0;
    sim.params.center_strength = 0.0;
    sim
}

/// **The gravity skip is a decision, and this is the arithmetic behind it.** A velocity of
/// `-0.0` is a real value, and adding `+0.0` to it gives `+0.0` — a different `f64`, and a
/// different byte. At `gravity == 0` the tick must therefore *skip* the force rather than
/// apply a zero strength; [`gravity::apply`](crate::layout::force::session::gravity)'s own
/// test is where that flip is pinned, on the function rather than through the tick.
///
/// Through the tick the distinction is **not** observable today, and the reason is worth
/// writing down rather than discovering later: every node's velocity passes through an
/// addition in the many-body force on every tick — the force always contributes, at the
/// very least `±0.0` past `distanceMax` — and IEEE addition yields `-0.0` only from a sum
/// in which *both* operands are `-0.0`, which no state reachable from a session (every
/// velocity starts at `+0.0`) can be. So the guard is the specified contract and
/// insurance, not a difference today's 65 goldens can see. The recommended resolution is
/// in the milestone's report, not here.
#[test]
fn a_tick_with_no_pins_and_no_gravity_leaves_the_columns_where_they_were() {
    let mut sim = quiet(0.0);
    sim.x[0] = -40.0;
    sim.vx[0] = -0.0;
    tick(&mut sim);
    assert_eq!(
        sim.x[0].to_bits(),
        (-40.0f64).to_bits(),
        "no force moved it"
    );
    assert_eq!(
        sim.vx[0].to_bits(),
        0.0f64.to_bits(),
        "and the many-body zero landed on +0.0"
    );
}

/// The same state with gravity **on**, so the test above is not passing because the sign is
/// unreachable: a positive strength moves it.
#[test]
fn a_tick_with_gravity_does_move_that_velocity() {
    let mut sim = quiet(0.5);
    sim.x[0] = -40.0;
    sim.vx[0] = -0.0;
    tick(&mut sim);
    let params = sim.params;
    let alpha: f64 = params.initial_alpha + (0.0 - params.initial_alpha) * params.alpha_decay;
    let want: f64 = (0.0 - -40.0) * params.gravity * alpha * params.velocity_decay;
    assert_eq!(
        sim.vx[0].to_bits(),
        want.to_bits(),
        "(0 - x) * gravity * alpha, then velocity decay"
    );
    assert_eq!(
        sim.x[0].to_bits(),
        (-40.0 + want).to_bits(),
        "then integrated"
    );
}

/// A pin is *placed*, not integrated (`simulation.js:53-56`): both the position and the
/// velocity, with the velocity zeroed so that unpinning starts from rest rather than from
/// whatever the pin was hiding.
#[test]
fn a_pinned_axis_is_placed_and_its_velocity_is_zeroed() {
    let mut sim = quiet(0.0);
    sim.fx[0] = Some(3.0);
    sim.fy[0] = Some(-4.0);
    sim.vx[0] = 99.0;
    sim.vy[0] = -99.0;
    tick(&mut sim);
    assert_eq!((sim.x[0], sim.y[0]), (3.0, -4.0), "placed at the pin");
    assert_eq!(
        (sim.vx[0], sim.vy[0]),
        (0.0, 0.0),
        "and the velocity is gone"
    );
}

/// `alpha_target` is applied in d3's line (`simulation.js:45`), so a non-zero target moves
/// alpha *toward the target* rather than toward zero — which is the whole difference
/// between a live layout and a countdown.
#[test]
fn alpha_moves_toward_its_target_and_not_always_toward_zero() {
    let mut sim = quiet(0.0);
    sim.alpha = 1.0;
    sim.alpha_target = 0.4;
    tick(&mut sim);
    let want: f64 = 1.0 + (0.4 - 1.0) * 0.06;
    assert_eq!(sim.alpha.to_bits(), want.to_bits());
    assert!(
        sim.alpha < 1.0 && sim.alpha > 0.4,
        "between the target and where it was"
    );
}

/// Replacing the parameters recomputes the per-link geometry and nothing else: the columns,
/// the velocities and the pins are the run's, not the parameters'.
#[test]
fn set_params_keeps_the_run_and_rebuilds_the_link_geometry() {
    let nodes = [node("a", ""), node("b", "")];
    let edges = [crate::records::build::edge("e", "a", "b")];
    let topology = index_model(&nodes, &edges).expect("fits");
    let mut sim = Sim::new(&topology, LiveParams::from(ForceParams::default()), 0);
    for _ in 0..3 {
        tick(&mut sim);
    }
    let (x, distance) = (sim.x.clone(), sim.link_distance.clone());
    sim.set_params(LiveParams {
        link_distance: 120.0,
        ..LiveParams::default()
    });
    // A link's own distance is the base over `max(0.4, edge.strength)` (link.rs).
    let own = |base: f64| vec![base / f64::max(0.4, edges[0].strength)];
    assert_eq!(sim.x, x, "positions are the run's, not the parameters'");
    assert_eq!(
        sim.link_distance,
        own(120.0),
        "and the link geometry is rebuilt"
    );
    assert_eq!(distance, own(60.0), "from the frozen 60, not from nothing");
}

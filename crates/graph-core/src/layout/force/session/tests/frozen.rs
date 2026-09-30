//! **The frozen acceptance: a session in which every node is frozen does not move.**
//!
//! `m1d.rs` pins *one* node and says a pin is exact. That leaves the composed case
//! uncovered: a caller who holds every node — a saved layout on screen, a "show me this
//! arrangement again" button — has nowhere to look, because one node at rest is also what
//! a layout whose forces are all dead would give. So this module pins **every row, on both
//! axes**, and compares the whole position snapshot bit for bit across a run of ticks.
//! `to_bits()` throughout: a one-ULP drift is a failure, because a pin that is *nearly*
//! exact is the same bug a pin that is exactly one ULP out has.
//!
//! Three claims, in the order they earn their keep:
//!
//! 1. every row pinned ⇒ not one bit of the snapshot changes, over four gate seeds;
//! 2. the same seeds with `unpin_all` ⇒ the snapshot *does* change, which is what stops
//!    (1) from passing on a `pin` that does nothing at all;
//! 3. `reheat(0.0)` ⇒ still nothing moves, on a session whose only remaining force
//!    carries an `alpha` factor — and the exclusions that claim needs, spelled out at the
//!    test that uses them.

use super::support;
use crate::layout::force::session::{ForceSession, LiveParams, NodeRow};

/// The gate seeds the acceptance is claimed over: the first three of `prompt.md` §7.1's
/// range, plus `7` so the claim is not carried entirely by small models.
const SEEDS: [u32; 4] = [0, 1, 2, 7];

/// `row`'s frozen point: index-derived, exact `f64` arithmetic, one value per axis and a
/// different value for every row.
///
/// Deliberately not the origin. Every row at one point would take the coincidence guard's
/// `jiggle` branch in `link::halves` and `collide::resolve` (`dx == 0.0`), so a frozen
/// layout of that shape would only ever prove the guard was wired up — a strictly weaker
/// claim than "held apart and still did not move".
fn pin_point(row: u32) -> (f64, f64) {
    let i = f64::from(row);
    (i * 5.5 - 17.25, i * i * 0.5 - 3.5)
}

/// The two position columns a frozen session over `rows` nodes starts and stays on.
fn pin_columns(rows: u32) -> (Vec<f64>, Vec<f64>) {
    let mut xs = Vec::with_capacity(rows as usize);
    let mut ys = Vec::with_capacity(rows as usize);
    for row in 0..rows {
        let (x, y) = pin_point(row);
        xs.push(x);
        ys.push(y);
    }
    (xs, ys)
}

/// A default session over `seed`'s gate model, sitting on [`pin_point`] for every row and
/// pinned there on **both** axes.
///
/// Started from `from_positions` rather than from the golden spiral so that the snapshot
/// the test takes *is* the pinned state already: `pin` records `fx`/`fy` and the node only
/// arrives at them at the next tick's integration tail (`simulation.js:53-56`), so a
/// snapshot taken before the first tick would be the spiral and every row would "move" into
/// its pin for reasons that have nothing to do with the pin.
fn frozen_everywhere(seed: u32) -> ForceSession {
    let topology = support::topology(seed);
    let rows = topology.node_count();
    let (xs, ys) = pin_columns(rows);
    let mut session = ForceSession::from_positions(&topology, LiveParams::default(), &xs, &ys)
        .expect("one finite value per node");
    for row in 0..rows {
        session
            .pin(NodeRow::new(row), xs[row as usize], ys[row as usize])
            .expect("every row of the snapshot is nameable");
    }
    session
}

/// The acceptance itself: with every axis of every row pinned, sixteen ticks leave the
/// snapshot bit-for-bit identical. `Sim::integrate`'s pin branch writes `x[i] = fx` and
/// zeroes `vx`, so no force can reach a position through this path — and the ways it could
/// anyway are the ones this catches: `center()` writes `x` directly rather than adding to
/// it, `collide` folds into `vx`, and a pin that were honoured on `x` but not `y` would
/// still half-move every node. Both axes are pinned and both axes are compared.
#[test]
fn a_session_with_every_row_pinned_does_not_move_a_single_bit() {
    for seed in SEEDS {
        let mut session = frozen_everywhere(seed);
        let before = support::bits(&session);
        session.step(16);
        assert_eq!(
            support::bits(&session),
            before,
            "seed {seed}: every row is held, so every axis keeps its exact bit pattern"
        );
    }
}

/// The negative control for the test above: the same seeds, the same pins, the same ticks
/// — with only `unpin_all` in between. If the layout stayed still here, the acceptance
/// would be evidence that a `pin` does nothing rather than that a pin holds, and the
/// assertion above would be satisfied by a broken motor.
#[test]
fn releasing_every_pin_on_the_same_seeds_moves_the_layout() {
    for seed in SEEDS {
        let mut session = frozen_everywhere(seed);
        session.unpin_all();
        let before = support::bits(&session);
        session.step(16);
        assert_ne!(
            support::bits(&session),
            before,
            "seed {seed}: with the pins gone the forces have the layout again"
        );
    }
}

/// At `alpha` 0 nothing that is scaled by `alpha` can move a node — and on this parameter
/// set nothing else is left, so the snapshot holds.
///
/// The two forces excluded from the claim are excluded *deliberately*:
///
/// * `center` (`barnes_hut/sim.rs`) carries no `alpha` factor at all — d3's `center.js`
///   `force()` takes no `alpha`, and this motor's `center()` subtracts the mean from
///   `x`/`y` directly. At `alpha` 0 a *default* session still moves for that reason, so
///   "alpha 0 means motionless" is false as a general claim and this test does not make
///   it; `center_strength: 0.0` is what removes it here, and `x[i] -= 0.0` is the exact
///   `x[i]` for every `x[i]` a session can hold.
/// * `collide` (`barnes_hut/collide.rs`) is likewise unscaled by `alpha` (d3's
///   `collide.js` takes no `alpha`), so it too still acts at 0;
///   `collide_radius: 0.0` makes its reach `0`, every overlap test `l >= 0` true, and the
///   pass an exact no-op — a no-op it must be stated to be, not assumed.
///
/// `charge: 0.0` is belt and braces: the many-body force *is* `alpha`-scaled
/// (`charge.rs`), so `alpha` 0 alone would suffice, and a zero charge means the claim does
/// not rest on `0.0 * m / l` rounding to a zero delta. What is left is `link`, the only
/// force left in `Sim::tick` that could move anything, and `gravity: 0.0` skips the force
/// outright rather than applying a zero strength.
#[test]
fn alpha_at_zero_leaves_a_layout_whose_only_forces_carry_alpha_where_it_is() {
    let params = LiveParams {
        collide_radius: 0.0,
        center_strength: 0.0,
        charge: 0.0,
        ..LiveParams::default()
    };
    for seed in SEEDS {
        let topology = support::topology(seed);
        let mut session =
            ForceSession::new(&topology, params).expect("all three zeroes are in range");
        session.reheat(0.0).expect("0.0 is in range");
        let before = support::bits(&session);
        session.step(8);
        assert_eq!(
            support::bits(&session),
            before,
            "seed {seed}: alpha 0 scales the one force left that could have moved a node"
        );
    }
}

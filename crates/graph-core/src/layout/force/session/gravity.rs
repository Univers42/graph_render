//! Gravity: d3's `forceX(0) + forceY(0)`, one force applied to both axes.
//!
//! `x.js:13` (and `y.js:13`, the same line for the other axis) is the whole of it:
//! `node.vx += (xz[i] - node.x) * strengths[i] * alpha`, left to right, with `xz[i] = 0`
//! and `strengths[i] = gravity`. The order is not decoration — `(0 - x) * g * alpha` and
//! `alpha * ((0 - x) * g)` are the same real number and not always the same `f64`, and
//! this motor is gated on bits.
//!
//! The tick **skips** this function when `gravity == 0` rather than calling it with a zero
//! strength; [`applying_it_at_zero_is_not_the_same_bytes`] is why, and it is a test in this
//! file because the skip lives in `sim.rs` and the reason belongs next to the arithmetic.

use crate::layout::force::barnes_hut::sim::Sim;

/// `forceX(0) + forceY(0)` at `sim.params.gravity`, in `x.js`'s own order.
pub(crate) fn apply(sim: &mut Sim) {
    let gravity = sim.params.gravity;
    let alpha = sim.alpha;
    for i in 0..sim.x.len() {
        sim.vx[i] += (0.0 - sim.x[i]) * gravity * alpha;
        sim.vy[i] += (0.0 - sim.y[i]) * gravity * alpha;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{empty_model, index_model};
    use crate::layout::force::{ForceParams, LiveParams};
    use crate::records::build::node;

    fn sim_with(gravity: f64) -> Sim {
        let nodes = [node("a", ""), node("b", "")];
        let topology = index_model(&nodes, &[]).expect("fits");
        let mut sim = Sim::new(&topology, LiveParams::from(ForceParams::default()), 0);
        sim.params.gravity = gravity;
        sim
    }

    /// **The reason the tick skips the force at zero strength.** `(0 - x) * 0.0` is `+0.0`
    /// for every `x < 0` and `-0.0` for every `x > 0`, and adding a `+0.0` to a `-0.0`
    /// velocity gives `+0.0` — which then reaches the position, where `-0.0` and `+0.0`
    /// are different `f64` values with different bytes. So "apply a force of zero" is not
    /// "apply no force", and the frozen layout's 4-way hash is only safe because the
    /// former does not happen.
    #[test]
    fn applying_it_at_zero_is_not_the_same_bytes() {
        let mut sim = sim_with(0.0);
        sim.x[0] = -40.0;
        sim.vx[0] = -0.0;
        sim.x[1] = 40.0;
        sim.vx[1] = -0.0;
        apply(&mut sim);
        assert_eq!(
            sim.vx[0].to_bits(),
            0.0f64.to_bits(),
            "a negative x contributes +0.0, which turns -0.0 into +0.0"
        );
        assert_eq!(
            sim.vx[1].to_bits(),
            (-0.0f64).to_bits(),
            "a positive x contributes -0.0, which leaves -0.0 alone"
        );
    }

    /// And the tick really does skip it: a session at `gravity = 0` is the frozen layout's
    /// own bytes (`session/tests/live.rs`).
    #[test]
    fn the_force_adds_nothing_that_is_not_the_formula() {
        let mut sim = sim_with(0.25);
        sim.x[0] = 10.0;
        sim.alpha = 0.5;
        apply(&mut sim);
        let want: f64 = (0.0 - 10.0) * 0.25 * 0.5;
        assert_eq!(
            sim.vx[0].to_bits(),
            want.to_bits(),
            "velocity, left to right"
        );
        assert_eq!(
            sim.vy[0].to_bits(),
            0.0f64.to_bits(),
            "y starts at zero and stays put"
        );
    }

    /// A session over an empty topology has no columns and the force must not index one.
    #[test]
    fn an_empty_simulation_is_left_alone() {
        let mut sim = Sim::new(&empty_model(), LiveParams::from(ForceParams::default()), 0);
        apply(&mut sim);
        assert!(sim.vx.is_empty());
    }
}

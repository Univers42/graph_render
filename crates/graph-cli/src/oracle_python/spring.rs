//! `layout.force.spring` against networkx 3.6's own `spring_layout` at `dim=2`
//! (`harness/oracle-spring.py`), run in the `ge-python-oracle` image.
//!
//! **A coordinate gap would measure the chaos, not the port.** The two arms start from
//! different positions — ours from graph-core's seeded Mulberry32, networkx's from numpy's
//! `RandomState` — and diverge inside the first step, exactly as FA2 does
//! (`harness/oracle-fa2.py`), whose arm escapes the same problem the other way: it hands
//! networkx the port's own initial positions so the arms differ only in arithmetic. This
//! one cannot, because `spring_layout` picks its own start and the start *is* the claim.
//!
//! **So the metric is the stress correlation, and it is computed once, here.** Both arms'
//! positions go through [`crate::stress::metric::correlate`] — the same function the d3
//! arm and `graph-cli stress` use (`stress/metric.rs`), which is why the harness computes
//! no metric of its own. Each arm is scored separately over 32 max-min pivots; ours
//! divided by networkx's would be a ratio of two numbers that may both be negative, so
//! what is gated is the **deficit** `max(0, theirs - ours)`: zero whenever our drawing
//! respects the graph's distances at least as well as the reference's, positive exactly
//! when it does not. "Different, but not worse", falsifiable from both ends.
//!
//! **Which statistic carries the claim, and why.** The ceiling is on the **median** deficit
//! over the 1000 gate seeds, and the worst, the 90th percentile and the worst case strictly
//! below 500 are printed and recorded beside it so a reader can see the whole shape. The
//! reason is that the per-seed statistic is a noisy sample: on a small graph, 50
//! iterations from a random start is far from settled, so both arms' correlations are
//! start-dependent and the extreme of 1000 such samples is tail, not port quality. The
//! median is the robust summary of the same 1000 samples, and [`CEILING`] is the next power
//! of ten above the median measured (7.288e-3), which is the rule every other ceiling in
//! the registry follows. Gating the tail instead would need a ceiling so loose it could not
//! be falsified: `GM_MUTATE_SPRING_ITERATIONS` at 3 moves the worst from 1.861e-1 to only
//! 6.738e-1, while it moves the median from 7.288e-3 to 4.216e-1, a factor of 58.
//!
//! **What the ceiling does not stand in for.** Two assertions here are exact and are worth
//! more than any statistic:
//!
//! - **the rescale contract** (`layout.py:646`): every layout with two or more nodes is
//!   centred and spans exactly `scale`, whatever the start was. Checked on *both* arms,
//!   one part in a million, because ours arrives already narrowed to `f32` by the snapshot.
//! - **a one-node graph** is the origin on both arms (`layout.py:618-624`).
//!
//! And the analytically-determined small cases — one node, two nodes, a path, a star —
//! are asserted *exactly* in graph-core's own tests, because there the answer is closed
//! and no statistic can replace it.
//!
//! Ponytail: a stress ratio cannot see a mirrored or rotated but otherwise equivalent
//! embedding — it certifies distances and says nothing about orientation, which is the
//! strongest true claim a force layout admits. The `n >= 500` fork (networkx's
//! `method="auto"` hands those to an L-BFGS energy minimiser this port does not reproduce,
//! `layout.py:140-141`) is inside the compared set, so the worst case *below* 500 is
//! printed beside the worst overall and the ceiling can be read either way. Neither arm is
//! asked about a disconnected graph or an empty one; those rest on graph-core's own tests.

pub mod ingest;

use super::{Differential, points};
use graph_core::layout::force::spring::{Spring, SpringParams};
use graph_core::{REFERENCE_DEGREE, Stage, gate_node_count, index_model, seeded_model};
use serde_json::{Value, json};

pub const SPRING: Differential = Differential {
    name: "spring",
    ceilings: &[("layout.force.spring", "spring", CEILING)],
    line,
};

/// The **median** stress deficit's ceiling: the next power of ten above the median
/// `max(0, theirs - ours)` measured over the 1000 gate seeds (7.288e-3, against a 90th
/// percentile of 6.950e-2 and a worst of 1.861e-1 at seed 17;
/// `docs/measurements/p12-t2.md` prints all of them). The median and not the worst, because
/// the worst of 1000 noisy small-graph samples is tail — a ceiling that admits the measured
/// tail would be so loose that a port run at a third of the reference's iteration budget
/// still passed it, which is exactly what `scripts/orch/rows/p12-t2.rows`'s
/// `oracle-spring-mutated` row measures.
pub(crate) const CEILING: f64 = 1e-1;

/// One seed's line. `--max-iter` overrides the differential's own iteration budget, the
/// escape hatch `docs/measurements/p12-t2.md` re-measures the whole comparison with.
fn line(seed: u32, max_iter: Option<u32>) -> Result<Value, String> {
    let n = gate_node_count(seed);
    let (nodes, edges) = seeded_model(seed, n, REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).map_err(|e| e.to_string())?;
    let columns = topology.edges();
    // What the arm is told to run, and what this arm actually runs. They differ exactly
    // when a control is set: the fixture must keep describing the reference, or the
    // control would move the oracle too and measure nothing.
    let reference = params(max_iter);
    let ours = port_params(reference, crate::hashgate::spring_perturbation()?);
    let geometry = Spring::run(&topology, &ours).map_err(|e| e.to_string())?;
    Ok(json!({
        "seed": seed, "n": n, "source": columns.source, "target": columns.target,
        "params": {
            "iterations": reference.iterations, "threshold": reference.threshold,
            "scale": reference.scale,
        },
        "spring": points("layout.force.spring", &geometry.nodes)?,
    }))
}

/// What networkx is to be run with: the differential's own budget, or `--max-iter`'s
/// override. Never the perturbation — the fixture describes the *reference*, so a perturbed
/// port is measured against the unperturbed algorithm and the gap is what it should be.
fn params(max_iter: Option<u32>) -> SpringParams {
    SpringParams {
        iterations: max_iter.unwrap_or(SpringParams::default().iterations),
        ..SpringParams::default()
    }
}

/// The port's parameters: `reference` with the knob's whole [`SpringParams`] laid over it,
/// so `GM_MUTATE_SPRING_ITERATIONS` moves the port and the recorded fixture does not. `None`
/// is the honest run.
///
/// **The whole struct, not one field of it.** Laying only `iterations` over `reference` and
/// leaving `threshold` and `scale` alone would be a control that moves one thing by
/// accident and two by design, and a later knob added to [`SpringParams`] would be silently
/// inert. `scripts/orch/rows/p12-t2.rows`'s `oracle-spring-mutated` row is what holds this:
/// a control that leaves the drawing unchanged must leave the gate green, and that row
/// requires red.
pub(super) fn port_params(reference: SpringParams, knob: Option<SpringParams>) -> SpringParams {
    knob.unwrap_or(reference)
}

#[cfg(test)]
mod tests {
    use super::{SpringParams, port_params};

    fn at(iterations: u32) -> SpringParams {
        SpringParams {
            iterations,
            ..SpringParams::default()
        }
    }

    #[test]
    fn no_control_leaves_both_arms_on_the_differentials_own_budget() {
        assert_eq!(port_params(at(50), None), at(50));
    }

    #[test]
    fn a_control_moves_the_port_and_nothing_else() {
        assert_eq!(port_params(at(50), Some(at(3))), at(3));
    }

    /// The regression this function exists for: a knob applied to a struct and then
    /// overwritten field by field is a control that reports green and changed nothing.
    #[test]
    fn a_control_at_the_default_value_is_still_a_control() {
        assert_eq!(
            port_params(at(3), Some(SpringParams::default())),
            SpringParams::default()
        );
    }
}

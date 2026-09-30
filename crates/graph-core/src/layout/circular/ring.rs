//! `layout.circular.ring`: every node on the unit circle in dense-index order, edges
//! ignored. Reference: networkx 3.6 `circular_layout`
//! (`networkx/drawing/layout.py:129`): angle `k * 2*pi / n` for node `k` (the duplicate
//! `2*pi` endpoint dropped), then `rescale_layout`. A lone node sits at the centre.
//!
//! Not `layout.circular.radial` (`super`), which is a tree radial over the repaired
//! hierarchy; the two share nothing but the word "circular".
//!
//! Angles are `f64` where networkx narrows them to `f32`, so the differential against it
//! is a tolerance (1e-6), never bytes. Nothing here is a heuristic.
//!
//! Phase 11: the per-node gather is handed to a [`Runner`] and `coords`' shared merge
//! stays serial, so a width is a *schedule* of this computation and not a second one. The
//! merge is shared with `spiral`, `bipartite` and `random` — one float sum, one copy.

use super::super::Geometry;
use super::super::coords::{point_geometry, rescale_under};
use crate::exec::{Runner, Serial, StepRange};
use crate::index::Topology;
use crate::stage::StageError;
use std::ops::Range;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.circular.ring";

/// Runs the ring layout; never refuses.
///
/// The serial tier: [`run_with`] over [`Serial`] with one worker, and the arm every other
/// runner must hash-equal.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    run_with(topology, &Serial, 1)
}

/// The same ring, its per-node gather handed to `runner` over `workers` workers.
///
/// The angle, the step and the trigonometry are the same code the serial stage runs, and
/// only the division of the gather differs — so `run_with(..., &Serial, 1)` is [`run`] by
/// construction. `workers` below 2 is the serial path (see [`Runner`]), so a host with no
/// threads gets the same bytes rather than a fast failure.
pub fn run_with(
    topology: &Topology,
    runner: &impl Runner,
    workers: u32,
) -> Result<Geometry, StageError> {
    run_under(topology, runner, workers, false)
}

/// [`run_with`] with the shared merge's negative control reachable, so a host can run a
/// *deliberately wrong* tier and the gate must go red.
///
/// Separate from [`run_with`] rather than a defaulted argument on it, for
/// `BarnesHut::run_under`'s reason: a control a caller can forget to pass is not a
/// control, it is a second path nobody exercises.
pub fn run_under(
    topology: &Topology,
    runner: &impl Runner,
    workers: u32,
    split: bool,
) -> Result<Geometry, StageError> {
    let count = topology.node_count();
    if count < 2 {
        return Ok(point_geometry(
            &vec![0.0; count as usize],
            &vec![0.0; count as usize],
        ));
    }
    let mut pairs = Vec::new();
    runner.run(
        &Arc {
            count,
            step: 1.0 / f64::from(count),
        },
        workers,
        &mut pairs,
    );
    let (mut x, mut y): (Vec<f64>, Vec<f64>) = pairs.into_iter().unzip();
    rescale_under(&mut x, &mut y, split);
    Ok(point_geometry(&x, &y))
}

/// The ring's per-node gather: one `libm` cosine and sine for node `k` at `k * step * 2*pi`.
///
/// `Out` is the node's own pair, so one worker writes both of a node's coordinates. The
/// kernel reads nothing but its own `step` and the range it is given — no shared state, no
/// reduction, and no element whose value depends on another (D10), so any division of
/// `0..n` is a legal plan.
struct Arc {
    /// How many nodes the ring has — carried as the integer it is, because `1.0 / step` is
    /// not always the count back (a float round trip in `len` is a range-boundary bug that
    /// only shows at a width that does not divide it evenly).
    count: u32,
    step: f64,
}

impl StepRange for Arc {
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.count
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        for (k, slot) in range.zip(out) {
            let angle = f64::from(k) * self.step * (2.0 * core::f64::consts::PI);
            *slot = (libm::cos(angle), libm::sin(angle));
        }
    }
}

#[cfg(test)]
mod tests;

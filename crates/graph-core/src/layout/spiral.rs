//! `layout.spiral`: nodes along a 2D spiral in dense-index order, edges ignored.
//! Reference: networkx 3.6 `spiral_layout` (`networkx/drawing/layout.py:1242`), 2D only,
//! at its defaults `resolution = 0.35`, `equidistant = false`. SciGraphs' 2D dispatcher
//! never calls it (its `SPIRAL_3D` is a different, conical curve), so there is no
//! SciGraphs setting to follow: the defaults are networkx's.
//!
//! Archimedean (the default): `i * (cos, sin)(resolution * i)`. Equidistant: chord 1, radial step 0.5, so successive nodes are one chord apart along
//! the curve. Both are
//! then passed through `rescale_layout`. One node sits at the centre.
//!
//! Exact closed forms (`libm` trigonometry); the differential is a tolerance because
//! networkx evaluates in numpy's `f64`.
//!
//! Phase 11: **the archimedean branch is threaded and the equidistant branch is not.** The
//! first is a per-node gather, `i * (cos, sin)(resolution * i)`, and hands to a [`Runner`]
//! with no merge of its own; the second carries `theta` across iterations, so node `k`'s
//! angle is a function of every node before it and is *sequential by nature* — it is never
//! routed through a runner, in one `if`, and a runner that were given it could only compute
//! the same bytes on one thread.

use super::Geometry;
use super::coords::{point_geometry, rescale_under};
use crate::exec::{Runner, Serial, StepRange};
use crate::index::Topology;
use crate::stage::StageError;
use std::ops::Range;

pub mod spiral_3d;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.spiral";

/// The 3D arm's capability id, SciGraphs' own conical curve — a different layout from
/// [`ID`], not this one at another dimension (`spiral_3d`'s header).
pub use spiral_3d::ID_3D;

/// [`spiral_3d::run`], the registered entry point for `layout.spiral.3d`.
pub fn run_3d(topology: &Topology) -> Result<Geometry, StageError> {
    spiral_3d::run(topology)
}

/// networkx's default `resolution`.
const RESOLUTION: f64 = 0.35;

/// The spiral's parameters: the resolution, and which of networkx's two branches to take.
///
/// A struct rather than two arguments because the threaded entry point takes the *stage's*
/// parameters as one value, the shape `BarnesHut::run_with` has — and `equidistant` in
/// particular has to travel with `resolution`, since a runner can only ever be given the
/// pair that is safe to divide.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpiralParams {
    /// Radians per step in the archimedean branch, the start angle in the equidistant one.
    /// Finite and above 0.
    pub resolution: f64,
    /// networkx's `equidistant`: the chord-by-chord branch, which is serial and stays so.
    pub equidistant: bool,
}

impl Default for SpiralParams {
    /// networkx's defaults, and the stage the gate hashes.
    fn default() -> Self {
        Self {
            resolution: RESOLUTION,
            equidistant: false,
        }
    }
}

/// Runs the spiral at networkx's defaults (Archimedean).
///
/// The serial tier: [`run_with`] over [`Serial`] with one worker, and the arm every other
/// runner must hash-equal.
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    run_with(topology, &SpiralParams::default(), &Serial, 1)
}

/// The same spiral, its archimedean gather handed to `runner` over `workers` workers.
///
/// The distance, the angle and the trigonometry are the same code the serial stage runs,
/// and only the division of the gather differs — so `run_with(..., &Serial, 1)` is [`run`]
/// by construction, which is the claim the per-width hash-gate arms check.
pub fn run_with(
    topology: &Topology,
    params: &SpiralParams,
    runner: &impl Runner,
    workers: u32,
) -> Result<Geometry, StageError> {
    run_under(topology, params, runner, workers, false)
}

/// [`run_with`] with the shared merge's negative control reachable, so a host can run a
/// *deliberately wrong* tier and the gate must go red.
pub fn run_under(
    topology: &Topology,
    params: &SpiralParams,
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
    let (mut x, mut y) = if params.equidistant {
        // Sequential by nature, and never routed through a runner: `theta` carries across
        // iterations, so this branch is the serial pass whatever the width.
        equidistant_points(count, params.resolution)
    } else {
        archimedean_with(runner, workers, count, params.resolution)
    };
    rescale_under(&mut x, &mut y, split);
    Ok(point_geometry(&x, &y))
}

/// networkx's `equidistant` branch: `theta` advances by `chord / r` per node.
fn equidistant_points(count: u32, resolution: f64) -> (Vec<f64>, Vec<f64>) {
    const CHORD: f64 = 1.0;
    const STEP: f64 = 0.5;
    let mut theta = resolution;
    theta += CHORD / (STEP * theta);
    (0..count)
        .map(|_| {
            let radius = STEP * theta;
            theta += CHORD / radius;
            (radius * libm::cos(theta), radius * libm::sin(theta))
        })
        .unzip()
}

/// networkx's plain branch over `count` nodes, its gather divided by `runner`.
fn archimedean_with(
    runner: &impl Runner,
    workers: u32,
    count: u32,
    resolution: f64,
) -> (Vec<f64>, Vec<f64>) {
    let mut pairs = Vec::new();
    runner.run(&Archimedean { count, resolution }, workers, &mut pairs);
    pairs.into_iter().unzip()
}

/// The archimedean gather: distance `i` at angle `resolution * i`.
///
/// `Out` is the node's own pair, so one worker writes both of a node's coordinates. The
/// kernel reads only its own `resolution` and the range it is given: no element's value
/// depends on another's (D10), so any division of `0..count` is a legal plan.
struct Archimedean {
    count: u32,
    resolution: f64,
}

impl StepRange for Archimedean {
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.count
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        for (i, slot) in range.zip(out) {
            let distance = f64::from(i);
            let angle = self.resolution * distance;
            *slot = (distance * libm::cos(angle), distance * libm::sin(angle));
        }
    }
}

#[cfg(test)]
mod tests;

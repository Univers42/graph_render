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

use super::Geometry;
use super::coords::{point_geometry, rescale};
use crate::index::Topology;
use crate::stage::StageError;

/// The layout's capability id, which is also its hash-gate stage.
pub const ID: &str = "layout.spiral";

/// networkx's default `resolution`.
const RESOLUTION: f64 = 0.35;

/// Runs the spiral at networkx's defaults (Archimedean).
pub fn run(topology: &Topology) -> Result<Geometry, StageError> {
    run_with(topology, RESOLUTION, false)
}

/// Runs the spiral at `resolution` (radians per step in the plain branch, the start
/// angle in the equidistant one), equidistant or Archimedean. `resolution` is finite
/// and above 0.
pub fn run_with(
    topology: &Topology,
    resolution: f64,
    equidistant: bool,
) -> Result<Geometry, StageError> {
    let count = topology.node_count() as usize;
    if count < 2 {
        return Ok(point_geometry(&vec![0.0; count], &vec![0.0; count]));
    }
    let (mut x, mut y) = if equidistant {
        equidistant_points(count, resolution)
    } else {
        archimedean_points(count, resolution)
    };
    rescale(&mut x, &mut y);
    Ok(point_geometry(&x, &y))
}

/// networkx's `equidistant` branch: `theta` advances by `chord / r` per node.
fn equidistant_points(count: usize, resolution: f64) -> (Vec<f64>, Vec<f64>) {
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

/// networkx's plain branch: distance `i` at angle `resolution * i`.
fn archimedean_points(count: usize, resolution: f64) -> (Vec<f64>, Vec<f64>) {
    (0..count)
        .map(|i| {
            let distance = i as f64;
            let angle = resolution * distance;
            (distance * libm::cos(angle), distance * libm::sin(angle))
        })
        .unzip()
}

#[cfg(test)]
mod tests;

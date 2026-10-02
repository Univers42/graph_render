//! `layout.force.kamada_kawai_3d`: [`super::KamadaKawai`] at `dim = 3`.
//!
//! SciGraphs calls KK with `dim=3` (`igraph_layouts.py:99`), so this id — not the 2-D one — is
//! the row the conformance matrix compares. The descent is [`super::kk_kernel`] at `D = 3` and
//! the parameters are the 2-D stage's [`KkParams`] (default `maxiter` `50 n`, `epsilon` 0,
//! `kkconst` `n`); what this module owns is the one thing the kernel deliberately does not: the
//! start layout, which the spec makes dimension-specific.
//!
//! **The 3-D start is the sphere of the spec's table** (`docs/layouts/layout.force.kamada_kawai.md`,
//! "The 3D start: the sphere"), read here row by row: `z = -1` at row 0, `+1` at row `n-1`, and
//! `z = -1 + 2i / (n - 1)` with `phi += 3.6 / (sqrt(n) * r)` in between, `r = sqrt(1 - z z)`.
//! The first and last rows are branched on rather than guarded afterwards, which is what the spec
//! asks for: they have `r = 0` and the source's own division would be by zero. `phi` advances
//! only on interior rows, so the two poles sit at `(0, 0, -1)` and `(0, 0, +1)` whatever `n` is.
//! The whole sphere is then scaled by `0.36 * sqrt(n)`, the empirical start radius the spec
//! quotes.
//!
//! **Where this port is finite and igraph is not.** On the three-node path `gate-01`, igraph's
//! 3x3 Newton block is near-singular at all three vertices and one step overflows `f64`; the
//! kernel's singular-block guard makes the step zero instead, so this stage returns finite
//! geometry there. That is a reference defect recorded in `docs/measurements/scigraphs-conformance.md`
//! (`IGRAPH_KK`), not a disagreement about the algorithm, and
//! `tests::the_three_node_path_that_breaks_igraph_is_finite_here` holds it.

use crate::index::Topology;
use crate::layout::Geometry;
use crate::layout::force::kamada_kawai::KkParams;
use crate::layout::force::kk_kernel;
use crate::layout::force::simple_graph;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The 3-D layout's capability id, which is also its hash-gate stage. A separate id from the
/// 2-D stage's because a separate snapshot: the same descent from a different start, with a z
/// column, hashes differently and must never be confused with the 2-D one.
pub const ID_3D: &str = "layout.force.kamada_kawai_3d";

/// [`kk_kernel::descend`] at `D = 3` from [`sphere_start`], then a geometry with the z column
/// attached. The narrowing is `f64 -> f32` per column, the same cast the 2-D stage makes.
pub struct KamadaKawai3D;

impl Stage for KamadaKawai3D {
    type Params = KkParams;
    const ID: &'static str = ID_3D;

    fn run(topology: &Topology, params: &Self::Params) -> Result<Geometry, StageError> {
        let n = topology.node_count() as usize;
        let pos = kk_kernel::descend::<3>(&simple_graph(topology), n, sphere_start(n), params);
        if let Some(axis) = non_finite_axis(&pos) {
            return Err(StageError::NonFinite { column: AXES[axis] });
        }
        let column = |axis: usize| -> Vec<f32> { pos.iter().map(|p| p[axis] as f32).collect() };
        Ok(Geometry::in_space(
            NodeGeometry::Point {
                x: column(0),
                y: column(1),
            },
            EdgeGeometry::Line,
            Vec::new(),
            column(2),
        ))
    }
}

/// The spec's sphere, scaled by the spec's `0.36 * sqrt(n)`.
///
/// [`sphere_row`] is `pub(super)` so the unit-sphere row can be pinned against the spec's table
/// without dividing the radius back out again.
pub(super) fn sphere_start(n: usize) -> Vec<[f64; 3]> {
    let radius = 0.36 * libm::sqrt(n as f64);
    let mut phi = 0.0;
    (0..n)
        .map(|i| {
            let row = sphere_row(i, n, &mut phi);
            [row[0] * radius, row[1] * radius, row[2] * radius]
        })
        .collect()
}

/// One row of the unit sphere at index `i` of `n`, advancing `phi` on interior rows only.
pub(super) fn sphere_row(i: usize, n: usize, phi: &mut f64) -> [f64; 3] {
    if i == 0 {
        return [0.0, 0.0, -1.0];
    }
    if i == n - 1 {
        return [0.0, 0.0, 1.0];
    }
    let z = -1.0 + 2.0 * i as f64 / (n - 1) as f64;
    let r = libm::sqrt(1.0 - z * z);
    *phi += 3.6 / (libm::sqrt(n as f64) * r);
    [r * libm::cos(*phi), r * libm::sin(*phi), z]
}

/// The column names D9 reports a non-finite value under, in axis order.
const AXES: [&str; 3] = ["node.x", "node.y", "node.z"];

/// The first axis holding a non-finite coordinate, walked in axis order so the answer does not
/// depend on which of several bad axes comes first in the node order.
fn non_finite_axis<const D: usize>(pos: &[[f64; D]]) -> Option<usize> {
    (0..D).find(|&axis| pos.iter().any(|p| !p[axis].is_finite()))
}

#[cfg(test)]
#[path = "kamada_kawai_3d/tests.rs"]
mod tests;

//! Runs `ticks` ticks of the 3D simulation over `graph` from `start`, with `alpha` as the
//! initial heat.
//!
//! The 3D counterpart of `settle.rs`, and deliberately without its `Tier`: the 3D tick has
//! no range kernels to divide (see `sim3d.rs`'s header), so there is no runner to choose and
//! no control to split. One settle, one schedule, one set of bytes — which is what lets the
//! multilevel layout below hand every level the same call and know that nothing about the
//! level can change the arithmetic.

use super::sim3d::Sim3;
use crate::layout::force::SimpleGraph;
use crate::layout::force::params::ForceParams;

pub(in crate::layout::force) fn settle3d(
    graph: SimpleGraph,
    params: ForceParams,
    start: (Vec<f64>, Vec<f64>, Vec<f64>),
    (ticks, alpha): (u32, f64),
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let mut sim = Sim3::from_parts(graph, params.into(), 0, start);
    sim.alpha = alpha;
    for _ in 0..ticks {
        sim.tick();
    }
    (sim.x, sim.y, sim.z)
}
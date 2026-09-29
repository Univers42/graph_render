//! Entry points that let another layout drive the Barnes-Hut simulation over its own
//! graph and start positions (the multilevel `yifan_hu` reuses it per level), so there
//! is one tick implementation, not two.

use super::seed::golden_spiral;
use super::sim::Sim;
use crate::layout::force::SimpleGraph;
use crate::layout::force::params::ForceParams;

/// The engine's golden-angle start positions for `n` nodes.
pub(crate) fn golden_seed(n: u32) -> (Vec<f64>, Vec<f64>) {
    golden_spiral(n)
}

/// Runs `ticks` ticks over `graph` from `start`, with `alpha` as the initial heat.
pub(crate) fn settle(
    graph: SimpleGraph,
    params: ForceParams,
    start: (Vec<f64>, Vec<f64>),
    (ticks, alpha): (u32, f64),
) -> (Vec<f64>, Vec<f64>) {
    let mut sim = Sim::from_parts(graph, params, 0, start);
    sim.alpha = alpha;
    for _ in 0..ticks {
        sim.tick();
    }
    (sim.x, sim.y)
}

//! The link force as a Jacobi gather (devil C7): a shared `x`/`vx`/`y`/`vy` snapshot for
//! the whole pass (accumulated into `dvx`/`dvy`, merged only once every edge has been
//! read), and each edge's delta computed exactly once and split between its two
//! endpoints by their degree bias. Ported from
//! `/home/user/refs/npm/d3-force-3.0.0/src/link.js`, with one documented deviation:
//! d3's bias is `count[source]/(count[source]+count[target])`, `source`/`target` being
//! the raw graph's own arbitrary direction; our shared [`SimpleGraph`] (devil C6) has
//! already collapsed that direction, so "source" and "target" are redefined here as the
//! pair's lower and higher node index respectively — a fixed, deterministic stand-in,
//! not the original edge's own orientation.

use super::sim::Sim;
use crate::layout::force::SimpleGraph;
use crate::layout::force::params::ForceParams;
use crate::rng::jiggle;

const PASS_X: u32 = 0;
const PASS_Y: u32 = 1;

/// Each simple edge's fixed `(distance, strength, bias)` (`forceLayout.ts:206-207`):
/// the topology never changes across ticks, so neither do these.
pub(super) fn geometry(
    graph: &SimpleGraph,
    params: &ForceParams,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let m = graph.lo.len();
    let (mut distance, mut strength, mut bias) = (
        Vec::with_capacity(m),
        Vec::with_capacity(m),
        Vec::with_capacity(m),
    );
    for e in 0..m {
        let s = graph.strength[e];
        distance.push(params.link_distance / f64::max(0.4, s));
        strength.push(f64::min(0.7, params.link_strength_scale * s));
        let (dlo, dhi) = (
            f64::from(graph.degree(graph.lo[e])),
            f64::from(graph.degree(graph.hi[e])),
        );
        bias.push(dlo / (dlo + dhi));
    }
    (distance, strength, bias)
}

/// One Jacobi pass over every simple edge (`link.js`'s own `iterations` defaults to,
/// and here is fixed at, 1).
pub(super) fn apply(sim: &mut Sim) {
    sim.dvx.iter_mut().for_each(|v| *v = 0.0);
    sim.dvy.iter_mut().for_each(|v| *v = 0.0);
    for e in 0..sim.graph.lo.len() {
        edge_delta(sim, e);
    }
    for i in 0..sim.vx.len() {
        sim.vx[i] += sim.dvx[i];
        sim.vy[i] += sim.dvy[i];
    }
}

fn edge_delta(sim: &mut Sim, e: usize) {
    let (lo, hi) = (sim.graph.lo[e] as usize, sim.graph.hi[e] as usize);
    let mut dx = (sim.x[hi] + sim.vx[hi]) - (sim.x[lo] + sim.vx[lo]);
    let mut dy = (sim.y[hi] + sim.vy[hi]) - (sim.y[lo] + sim.vy[lo]);
    if dx == 0.0 {
        dx = jiggle(sim.seed, sim.tick_no, PASS_X, (lo as u32, hi as u32));
    }
    if dy == 0.0 {
        dy = jiggle(sim.seed, sim.tick_no, PASS_Y, (lo as u32, hi as u32));
    }
    let l = libm::sqrt(dx * dx + dy * dy);
    let factor = (l - sim.link_distance[e]) / l * sim.alpha * sim.link_strength[e];
    let (fx, fy) = (dx * factor, dy * factor);
    let b = sim.link_bias[e];
    sim.dvx[hi] -= fx * b;
    sim.dvy[hi] -= fy * b;
    sim.dvx[lo] += fx * (1.0 - b);
    sim.dvy[lo] += fy * (1.0 - b);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::layout::force::simple_graph;
    use crate::records::build::{edge, node};

    #[test]
    fn bias_favours_the_lower_degree_endpoint_moving_more() {
        // hub (many edges) -- mid (one edge): hub's degree dwarfs mid's.
        let nodes = [
            node("hub", ""),
            node("mid", ""),
            node("a", ""),
            node("b", ""),
            node("c", ""),
        ];
        let edges = [
            edge("e0", "hub", "mid"),
            edge("e1", "hub", "a"),
            edge("e2", "hub", "b"),
            edge("e3", "hub", "c"),
        ];
        let t = index_model(&nodes, &edges).expect("fits");
        let g = simple_graph(&t);
        let (_, _, bias) = geometry(&g, &ForceParams::default());
        // edge0 is (hub=lo=0, mid=hi=1): bias = degree(lo)/(degree(lo)+degree(hi)) = 4/5.
        assert!((bias[0] - 0.8).abs() < 1e-12);
    }
}

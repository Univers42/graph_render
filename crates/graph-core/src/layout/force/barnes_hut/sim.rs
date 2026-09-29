//! The simulation state every force reads and writes: dense positions/velocities, the
//! two Barnes-Hut quadtrees (one over current positions for charge, one over projected
//! `x+vx` positions for collide, matching `manyBody.js`/`collide.js`'s own quadtree
//! keys), and the per-pass scratch buffers link/collide gather into before merging
//! (devil C7's "double buffered": a pass reads last pass's `vx`/`vy` in full, never one
//! it has itself begun to overwrite — see `link.rs`, `collide.rs`).
//!
//! The pins and `alpha_target` live here rather than in the session because the tick tail
//! is what honours them (`simulation.js:45-57`): alpha decays first, the forces run in
//! registration order, and then each node is either *placed* at its `fx`/`fy` or
//! integrated. A session that applied pins after the tick would be a different simulation.

use super::seed::golden_spiral;
use crate::index::Topology;
use crate::layout::force::LiveParams;
use crate::layout::force::quadtree::Quadtree;
use crate::layout::force::session::gravity;
use crate::layout::force::{SimpleGraph, simple_graph};

#[cfg(test)]
mod tests;

pub(in crate::layout::force) struct Sim {
    pub(in crate::layout::force) graph: SimpleGraph,
    pub(in crate::layout::force) params: LiveParams,
    pub(in crate::layout::force) seed: u32,
    pub(in crate::layout::force) tick_no: u32,
    pub(in crate::layout::force) alpha: f64,
    /// d3's `alphaTarget` (`simulation.js:21`, applied at `:45`). Zero is the frozen
    /// layout's value and the only one that lets a run settle.
    pub(in crate::layout::force) alpha_target: f64,
    pub(in crate::layout::force) x: Vec<f64>,
    pub(in crate::layout::force) y: Vec<f64>,
    pub(in crate::layout::force) vx: Vec<f64>,
    pub(in crate::layout::force) vy: Vec<f64>,
    /// `node.fx`, `node.fy` (`simulation.js:54-56`).
    pub(in crate::layout::force) fx: Vec<Option<f64>>,
    pub(in crate::layout::force) fy: Vec<Option<f64>>,
    pub(in crate::layout::force) dvx: Vec<f64>,
    pub(in crate::layout::force) dvy: Vec<f64>,
    pub(in crate::layout::force) px: Vec<f64>,
    pub(in crate::layout::force) py: Vec<f64>,
    pub(in crate::layout::force) charge_tree: Quadtree,
    pub(in crate::layout::force) collide_tree: Quadtree,
    pub(in crate::layout::force) mass: Vec<f64>,
    pub(in crate::layout::force) comx: Vec<f64>,
    pub(in crate::layout::force) comy: Vec<f64>,
    pub(in crate::layout::force) order: Vec<u32>,
    pub(in crate::layout::force) link_distance: Vec<f64>,
    pub(in crate::layout::force) link_strength: Vec<f64>,
    pub(in crate::layout::force) link_bias: Vec<f64>,
}

impl Sim {
    pub(crate) fn new(topology: &Topology, params: LiveParams, seed: u32) -> Self {
        let graph = simple_graph(topology);
        let n = topology.node_count() as usize;
        let (x, y) = golden_spiral(n as u32);
        let (link_distance, link_strength, link_bias) = super::link::geometry(&graph, &params);
        Self {
            alpha: params.initial_alpha,
            alpha_target: 0.0,
            graph,
            params,
            seed,
            tick_no: 0,
            vx: vec![0.0; n],
            vy: vec![0.0; n],
            fx: vec![None; n],
            fy: vec![None; n],
            dvx: vec![0.0; n],
            dvy: vec![0.0; n],
            px: vec![0.0; n],
            py: vec![0.0; n],
            charge_tree: Quadtree::default(),
            collide_tree: Quadtree::default(),
            mass: Vec::new(),
            comx: Vec::new(),
            comy: Vec::new(),
            order: Vec::new(),
            x,
            y,
            link_distance,
            link_strength,
            link_bias,
        }
    }

    /// How many node columns there are — a [`NodeRow`](crate::layout::force::NodeRow)'s
    /// bounds.
    pub(crate) fn rows(&self) -> u32 {
        self.x.len() as u32
    }

    /// Replaces the parameters, recomputing only what depends on them: the per-link
    /// distance, strength and bias (`link.rs`'s `geometry`). The quadtrees and the
    /// position columns are a function of the topology and the run, not of the parameters,
    /// so they stay.
    pub(crate) fn set_params(&mut self, params: LiveParams) {
        let (distance, strength, bias) = super::link::geometry(&self.graph, &params);
        (self.link_distance, self.link_strength, self.link_bias) = (distance, strength, bias);
        self.params = params;
    }

    /// One tick: `alpha` decays first (`simulation.js:45`, in d3's own form — with
    /// `alphaTarget = 0` it is bit for bit `alpha += -alpha * alphaDecay`), then link,
    /// many-body, center, collide and gravity run in the engine's registration order
    /// (`forceLayout.ts`'s `.force("link",...).force("charge",...).force("center",...)
    /// .force("collide",...)`, with gravity after them: d3 runs forces in registration
    /// order, `simulation.js:47-49`, and an anchoring force belongs after the
    /// interactions it is damping), then velocities integrate into position.
    pub(crate) fn tick(&mut self) {
        self.alpha += (self.alpha_target - self.alpha) * self.params.alpha_decay;
        super::link::apply(self);
        super::charge::apply(self);
        self.center();
        super::collide::apply(self);
        if self.params.gravity > 0.0 {
            gravity::apply(self);
        }
        self.integrate();
        self.tick_no += 1;
    }

    /// `center.js`: shifts every position by the mean, toward the origin — position, not
    /// velocity, and with no `alpha` scaling (`center.js`'s `force()` takes no `alpha`).
    fn center(&mut self) {
        let n = self.x.len();
        if n == 0 {
            return;
        }
        let (sx, sy) = self
            .x
            .iter()
            .zip(&self.y)
            .fold((0.0, 0.0), |(a, b), (&x, &y)| (a + x, b + y));
        let strength = self.params.center_strength;
        let dx = (sx / n as f64) * strength;
        let dy = (sy / n as f64) * strength;
        for i in 0..n {
            self.x[i] -= dx;
            self.y[i] -= dy;
        }
    }

    /// `simulation.js:51-57`, verbatim in structure: a pinned axis is *placed* and its
    /// velocity zeroed, a free axis integrates. `node.x += node.vx *= velocityDecay` is
    /// the same two statements in the same order, so a session with no pins is
    /// bit-identical to the frozen layout.
    fn integrate(&mut self) {
        let decay = self.params.velocity_decay;
        for i in 0..self.x.len() {
            if let Some(fx) = self.fx[i] {
                self.x[i] = fx;
                self.vx[i] = 0.0;
            } else {
                self.vx[i] *= decay;
                self.x[i] += self.vx[i];
            }
            if let Some(fy) = self.fy[i] {
                self.y[i] = fy;
                self.vy[i] = 0.0;
            } else {
                self.vy[i] *= decay;
                self.y[i] += self.vy[i];
            }
        }
    }

    /// The capacity of every buffer the tick loop refills, for the test that a tick
    /// allocates nothing (`dsa-and-memory.md`): the quadtrees plus the per-pass scratch.
    #[cfg(test)]
    pub(crate) fn scratch_capacities(&self) -> Vec<usize> {
        vec![
            self.charge_tree.capacity(),
            self.collide_tree.capacity(),
            self.mass.capacity(),
            self.comx.capacity(),
            self.comy.capacity(),
            self.order.capacity(),
            self.dvx.capacity(),
            self.dvy.capacity(),
            self.px.capacity(),
            self.py.capacity(),
        ]
    }
}

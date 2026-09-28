//! The simulation state every force reads and writes: dense positions/velocities, the
//! two Barnes-Hut quadtrees (one over current positions for charge, one over projected
//! `x+vx` positions for collide, matching `manyBody.js`/`collide.js`'s own quadtree
//! keys), and the per-pass scratch buffers link/collide gather into before merging
//! (devil C7's "double buffered": a pass reads last pass's `vx`/`vy` in full, never one
//! it has itself begun to overwrite — see `link.rs`, `collide.rs`).

use super::seed::golden_spiral;
use crate::index::Topology;
use crate::layout::force::params::ForceParams;
use crate::layout::force::quadtree::Quadtree;
use crate::layout::force::{SimpleGraph, simple_graph};

pub(super) struct Sim {
    pub(super) graph: SimpleGraph,
    pub(super) params: ForceParams,
    pub(super) seed: u32,
    pub(super) tick_no: u32,
    pub(super) alpha: f64,
    pub(super) x: Vec<f64>,
    pub(super) y: Vec<f64>,
    pub(super) vx: Vec<f64>,
    pub(super) vy: Vec<f64>,
    pub(super) dvx: Vec<f64>,
    pub(super) dvy: Vec<f64>,
    pub(super) px: Vec<f64>,
    pub(super) py: Vec<f64>,
    pub(super) charge_tree: Quadtree,
    pub(super) collide_tree: Quadtree,
    pub(super) mass: Vec<f64>,
    pub(super) comx: Vec<f64>,
    pub(super) comy: Vec<f64>,
    pub(super) order: Vec<u32>,
    pub(super) link_distance: Vec<f64>,
    pub(super) link_strength: Vec<f64>,
    pub(super) link_bias: Vec<f64>,
}

impl Sim {
    pub(super) fn new(topology: &Topology, params: ForceParams, seed: u32) -> Self {
        let graph = simple_graph(topology);
        let n = topology.node_count() as usize;
        let (x, y) = golden_spiral(n as u32);
        let (link_distance, link_strength, link_bias) = super::link::geometry(&graph, &params);
        Self {
            alpha: params.initial_alpha,
            graph,
            params,
            seed,
            tick_no: 0,
            vx: vec![0.0; n],
            vy: vec![0.0; n],
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

    pub(super) fn positions(&self) -> (&[f64], &[f64]) {
        (&self.x, &self.y)
    }

    /// One tick: `alpha` decays first (`simulation.js`'s own order, `alphaTarget = 0`),
    /// then link, many-body, center and collide run in the engine's registration order
    /// (`forceLayout.ts`'s `.force("link",...).force("charge",...).force("center",...)
    /// .force("collide",...)`), then velocities integrate into position.
    pub(super) fn tick(&mut self) {
        self.alpha += -self.alpha * self.params.alpha_decay;
        super::link::apply(self);
        super::charge::apply(self);
        self.center();
        super::collide::apply(self);
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

    /// `simulation.js`'s own tick tail: `node.x += node.vx *= velocityDecay`.
    fn integrate(&mut self) {
        let decay = self.params.velocity_decay;
        for i in 0..self.x.len() {
            self.vx[i] *= decay;
            self.x[i] += self.vx[i];
            self.vy[i] *= decay;
            self.y[i] += self.vy[i];
        }
    }
}

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

/// The tier choice for one run, bundled so [`Sim::tick`] stays under the four-parameter cap
/// (`refactor-rust.md`) and so the scratch buffer travels with the runner that fills it — a
/// caller cannot pair a runner with another caller's buffer by accident.
pub(super) struct How<'a, R: crate::exec::Runner> {
    /// Who runs the ranges.
    pub(super) runner: &'a R,
    /// How many workers it may use.
    pub(super) workers: u32,
    /// The scratch the gather writes, reused across ticks.
    pub(super) deltas: &'a mut Vec<(f64, f64)>,
    /// The negative control: read the next node's delta too.
    pub(super) split_sum: bool,
}

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

    /// Node `i`'s own many-body delta, over the tree and aggregate already built, into a
    /// caller's reused walk stack.
    ///
    /// The serial loop `charge::apply` runs, exposed so the range kernel
    /// ([`super::step::Pass`]) and its tests call the same function rather than a copy of
    /// the walk. A second copy would be a second program, and the kernel's equality test
    /// would then compare two programs instead of two schedules.
    pub(super) fn node_delta(
        &self,
        i: u32,
        stack: &mut Vec<(u32, crate::layout::force::quadtree::Bounds)>,
    ) -> (f64, f64) {
        super::charge::node_delta_with(self, i, stack)
    }

    /// One tick with the many-body pass divided by `runner` over `workers` workers:
    /// `alpha` decays first (`simulation.js`'s own order, `alphaTarget = 0`), then link,
    /// many-body, center and collide run in the engine's registration order
    /// (`forceLayout.ts`'s `.force("link",...).force("charge",...).force("center",...)
    /// .force("collide",...)`), then velocities integrate into position.
    ///
    /// The **only** tier-sensitive line is the many-body pass: everything else — the decay,
    /// link, center, collide, integrate, and the pass order itself — is the same
    /// straight-line code, and the many-body pass's own prologue (tree build, bottom-up
    /// aggregate) is single-threaded in both. That is deliberate and it is the whole safety
    /// argument: the sequence of reads and writes per node is unchanged, so tick `t + 1`
    /// sees exactly what tick `t` left whether one thread or seven made it.
    ///
    /// `deltas` is the caller's scratch, reused across ticks so a steady-state run
    /// allocates nothing here — one buffer for the whole layout, not one per tick.
    pub(super) fn tick<R: crate::exec::Runner>(&mut self, how: &mut How<'_, R>) {
        let How {
            runner,
            workers,
            deltas,
            split_sum,
        } = how;
        let (runner, workers, split_sum) = (*runner, *workers, *split_sum);
        self.alpha += -self.alpha * self.params.alpha_decay;
        super::link::apply(self);
        super::charge::apply_with(self, runner, workers, deltas, split_sum);
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
    ///
    /// `pub(super)` rather than private because the phase-11 range kernel's tests drive the
    /// same pass sequence the tick does ([`crate::layout::force::barnes_hut::tests`]), and
    /// a test that rebuilt the tick in its own words would be testing its own arithmetic
    /// rather than the kernel's.
    pub(super) fn integrate(&mut self) {
        let decay = self.params.velocity_decay;
        for i in 0..self.x.len() {
            self.vx[i] *= decay;
            self.x[i] += self.vx[i];
            self.vy[i] *= decay;
            self.y[i] += self.vy[i];
        }
    }
}

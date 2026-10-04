//! The dense simulation state, ported field-for-field from networkx 3.6's
//! `forceatlas2_layout` (`networkx/drawing/layout.py:1604-1875` in the pinned networkx-3.6
//! source, the whole function), scoped to its default configuration: `linlog=False`,
//! `distributed_action=False`, `strong_gravity=False`, `adjust_sizes=False`, `dim=2`,
//! `weight=None` (every edge weight 1). `swing`/`traction` are carried **cumulatively
//! across iterations**, never reset inside the loop — that looks like a quirk, but it is
//! what the reference implementation actually does, and this is a port, not a rewrite.

use crate::index::Topology;
use crate::layout::force::{SimpleGraph, simple_graph};
use crate::rng::{Mulberry32, jiggle};

mod barnes_hut;

/// Parameters `forceatlas2_layout` exposes and this port keeps (`dim`, `linlog`,
/// `distributed_action`, `strong_gravity`, `node_mass`, `node_size`, `weight`,
/// `store_pos_as` are fixed at their default/unused value — a documented deviation,
/// scoping the port to the common case).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fa2Params {
    /// Iteration ceiling (networkx default 100).
    pub max_iter: u32,
    /// Jitter tolerance (networkx default 1.0).
    pub jitter_tolerance: f64,
    /// Repulsion scale (networkx default 2.0).
    pub scaling_ratio: f64,
    /// Gravitational pull to the centroid (networkx default 1.0).
    pub gravity: f64,
    /// Seeds this port's initial positions. networkx defaults to `None` (the numpy
    /// global RNG); graph-core has no global mutable state (D5), so this is fixed and
    /// explicit instead.
    pub seed: u32,
}

impl Default for Fa2Params {
    fn default() -> Self {
        Self {
            max_iter: 100,
            jitter_tolerance: 1.0,
            scaling_ratio: 2.0,
            gravity: 1.0,
            seed: 0,
        }
    }
}

pub(super) struct Fa2State {
    graph: SimpleGraph,
    params: Fa2Params,
    iter_no: u32,
    swing: f64,
    traction: f64,
    speed: f64,
    speed_efficiency: f64,
    x: Vec<f64>,
    y: Vec<f64>,
    mass: Vec<f64>,
    ux: Vec<f64>,
    uy: Vec<f64>,
    /// `Some` to repel over a quadtree ([`with_tree`](Fa2State::with_tree)), `None` for the
    /// dense pair loop.
    tree: Option<barnes_hut::Tree>,
}

impl Fa2State {
    pub(super) fn new(topology: &Topology, params: Fa2Params) -> Self {
        let graph = simple_graph(topology);
        let n = topology.node_count();
        let mass = (0..n).map(|v| f64::from(graph.degree(v)) + 1.0).collect();
        let (x, y) = initial_positions(n, params.seed);
        Self {
            graph,
            params,
            iter_no: 0,
            swing: 1.0,
            traction: 1.0,
            speed: 1.0,
            speed_efficiency: 1.0,
            ux: vec![0.0; n as usize],
            uy: vec![0.0; n as usize],
            x,
            y,
            mass,
            tree: None,
        }
    }

    /// Repels over a quadtree, O(n log n) per iteration, instead of the dense pair loop.
    pub(super) fn with_tree(mut self) -> Self {
        self.tree = Some(barnes_hut::Tree::default());
        self
    }

    pub(super) fn positions(&self) -> (&[f64], &[f64]) {
        (&self.x, &self.y)
    }

    /// Runs to `max_iter`, or fewer if the update shrinks below networkx's own
    /// `1e-10` early-exit.
    pub(super) fn run(&mut self) {
        for _ in 0..self.params.max_iter {
            if self.iterate() < 1e-10 {
                break;
            }
        }
    }

    fn iterate(&mut self) -> f64 {
        self.ux.iter_mut().for_each(|v| *v = 0.0);
        self.uy.iter_mut().for_each(|v| *v = 0.0);
        self.attraction();
        match self.tree.take() {
            Some(mut tree) => {
                self.repel_tree(&mut tree, barnes_hut::THETA2);
                self.tree = Some(tree);
            }
            None => self.repulsion(),
        }
        self.gravity();
        let (swing, traction) = self.swing_and_traction();
        self.swing += swing;
        self.traction += traction;
        let (s, t) = (self.swing, self.traction);
        self.estimate_factor(s, t);
        self.iter_no += 1;
        self.apply_update()
    }

    /// `attraction = -einsum('ijk,ij->ik', diff, A)`, `A` 0/1 and symmetric: for each
    /// simple edge, both endpoints pull toward each other by the same amount.
    fn attraction(&mut self) {
        for e in 0..self.graph.lo.len() {
            let (lo, hi) = (self.graph.lo[e] as usize, self.graph.hi[e] as usize);
            let dx = self.x[hi] - self.x[lo];
            let dy = self.y[hi] - self.y[lo];
            self.ux[lo] += dx;
            self.uy[lo] += dy;
            self.ux[hi] -= dx;
            self.uy[hi] -= dy;
        }
    }

    /// Dense O(n²) repulsion, one unordered pair at a time (`tmp/d2*scaling_ratio`).
    /// Deviation from the bare reference: an exact coincidence (`d2 == 0`, impossible
    /// on the diagonal but not off it) is nudged apart by the counter hash first, so no
    /// `Infinity`/`NaN` factor can reach a node (D9) — networkx's own dense form has no
    /// such guard.
    fn repulsion(&mut self) {
        let n = self.x.len();
        let k = self.params.scaling_ratio;
        for i in 0..n {
            for j in (i + 1)..n {
                let (dx, dy) = self.repel_delta(i as u32, j as u32);
                let d2 = dx * dx + dy * dy;
                let f = self.mass[i] * self.mass[j] / d2 * k;
                self.ux[i] += dx * f;
                self.uy[i] += dy * f;
                self.ux[j] -= dx * f;
                self.uy[j] -= dy * f;
            }
        }
    }

    fn repel_delta(&self, i: u32, j: u32) -> (f64, f64) {
        let mut dx = self.x[i as usize] - self.x[j as usize];
        let mut dy = self.y[i as usize] - self.y[j as usize];
        if dx == 0.0 && dy == 0.0 {
            dx = jiggle(self.params.seed, self.iter_no, 0, (i, j));
            dy = jiggle(self.params.seed, self.iter_no, 1, (i, j));
        }
        (dx, dy)
    }

    /// `gravities = -gravity * mass * unit_vec(pos - mean(pos))`, `strong_gravity=False`.
    fn gravity(&mut self) {
        let n = self.x.len();
        if n == 0 {
            return;
        }
        let (mut mx, mut my) = (0.0, 0.0);
        for i in 0..n {
            mx += self.x[i];
            my += self.y[i];
        }
        (mx, my) = (mx / n as f64, my / n as f64);
        for i in 0..n {
            let (px, py) = (self.x[i] - mx, self.y[i] - my);
            let norm = f64::sqrt(px * px + py * py);
            let (ux, uy) = if norm > 0.0 {
                (px / norm, py / norm)
            } else {
                (0.0, 0.0)
            };
            self.ux[i] -= self.params.gravity * self.mass[i] * ux;
            self.uy[i] -= self.params.gravity * self.mass[i] * uy;
        }
    }

    fn swing_and_traction(&self) -> (f64, f64) {
        let (mut swing, mut traction) = (0.0, 0.0);
        for i in 0..self.x.len() {
            let (sx, sy) = (self.x[i] - self.ux[i], self.y[i] - self.uy[i]);
            swing += self.mass[i] * f64::sqrt(sx * sx + sy * sy);
            let (tx, ty) = (self.x[i] + self.ux[i], self.y[i] + self.uy[i]);
            traction += 0.5 * self.mass[i] * f64::sqrt(tx * tx + ty * ty);
        }
        (swing, traction)
    }

    /// networkx's own `estimate_factor` helper, verbatim.
    fn estimate_factor(&mut self, swing: f64, traction: f64) {
        let n = self.x.len() as f64;
        let jt = self.params.jitter_tolerance;
        let opt_jitter = 0.05 * f64::sqrt(n);
        let min_jitter = f64::sqrt(opt_jitter);
        let min_speed_efficiency = 0.05;
        let other = f64::min(10.0, opt_jitter * traction / (n * n));
        let mut jitter = jt * f64::max(min_jitter, other);
        if swing / traction > 2.0 {
            if self.speed_efficiency > min_speed_efficiency {
                self.speed_efficiency *= 0.5;
            }
            jitter = f64::max(jitter, jt);
        }
        let target_speed = if swing == 0.0 {
            f64::INFINITY
        } else {
            jitter * self.speed_efficiency * traction / swing
        };
        if swing > jitter * traction {
            if self.speed_efficiency > min_speed_efficiency {
                self.speed_efficiency *= 0.7;
            }
        } else if self.speed < 1000.0 {
            self.speed_efficiency *= 1.3;
        }
        self.speed += f64::min(target_speed - self.speed, 0.5 * self.speed);
    }

    fn apply_update(&mut self) -> f64 {
        let mut moved = 0.0;
        for i in 0..self.x.len() {
            let norm = f64::sqrt(self.ux[i] * self.ux[i] + self.uy[i] * self.uy[i]);
            let factor = self.speed / (1.0 + f64::sqrt(self.speed * self.mass[i] * norm));
            let (dx, dy) = (self.ux[i] * factor, self.uy[i] * factor);
            self.x[i] += dx;
            self.y[i] += dy;
            moved += f64::abs(dx) + f64::abs(dy);
        }
        moved
    }
}

/// networkx's own initial positions are `nx.random_layout` (uniform in the unit square,
/// `numpy`'s RNG); this port uses the crate's one sequential generator, `Mulberry32`,
/// seeded explicitly (devil C8's "two kinds, and only two" — `rng.rs`). Public so the
/// networkx differential can start the reference from the very same positions.
pub fn initial_positions(n: u32, seed: u32) -> (Vec<f64>, Vec<f64>) {
    let mut rng = Mulberry32::new(seed);
    let mut x = Vec::with_capacity(n as usize);
    let mut y = Vec::with_capacity(n as usize);
    for _ in 0..n {
        x.push(rng.next_f64());
        y.push(rng.next_f64());
    }
    (x, y)
}

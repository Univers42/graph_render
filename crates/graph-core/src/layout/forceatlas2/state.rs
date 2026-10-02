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
    /// How many coordinates every node carries: `2`, or `3` for
    /// `layout.forceatlas2.3d`.
    ///
    /// networkx's own default is `dim = 2` (`layout.py:1619`) and SciGraphs asks for
    /// `dim = 3` (`forceatlas.py:153`), so this is the parameter the two dispatcher arms
    /// differ on.
    pub dim: usize,
}

impl Default for Fa2Params {
    fn default() -> Self {
        Self {
            max_iter: 100,
            jitter_tolerance: 1.0,
            scaling_ratio: 2.0,
            gravity: 1.0,
            seed: 0,
            dim: 2,
        }
    }
}

/// The widest point this port keeps; see
/// [`crate::layout::force::fruchterman_reingold::MAX_DIM`].
pub const MAX_DIM: usize = 3;

pub(super) struct Fa2State {
    graph: SimpleGraph,
    params: Fa2Params,
    dim: usize,
    iter_no: u32,
    swing: f64,
    traction: f64,
    speed: f64,
    speed_efficiency: f64,
    /// The positions, one row of `dim` per node. SoA at `dim` 2 became one array here:
    /// networkx's own arrays are `(n, dim)` throughout (`layout.py:1729-1731`), so the
    /// row-major shape is the reference's, and at `dim` 2 the sums below run over the same
    /// two columns in the same order.
    p: Vec<[f64; MAX_DIM]>,
    mass: Vec<f64>,
    /// The per-iteration force accumulator, the `(n, dim)` array networkx calls `update`.
    u: Vec<[f64; MAX_DIM]>,
}

impl Fa2State {
    pub(super) fn new(topology: &Topology, params: Fa2Params, dim: usize) -> Self {
        let graph = simple_graph(topology);
        let n = topology.node_count();
        let mass = (0..n).map(|v| f64::from(graph.degree(v)) + 1.0).collect();
        let p = initial_positions(n, params.seed, dim);
        Self {
            graph,
            params,
            dim,
            iter_no: 0,
            swing: 1.0,
            traction: 1.0,
            speed: 1.0,
            speed_efficiency: 1.0,
            u: vec![[0.0; MAX_DIM]; n as usize],
            p,
            mass,
        }
    }

    pub(super) fn positions(&self) -> &[[f64; MAX_DIM]] {
        &self.p
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
        self.u.fill([0.0; MAX_DIM]);
        self.attraction();
        self.repulsion();
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
        let dim = self.dim;
        for e in 0..self.graph.lo.len() {
            let (lo, hi) = (self.graph.lo[e] as usize, self.graph.hi[e] as usize);
            for a in 0..dim {
                let d = self.p[hi][a] - self.p[lo][a];
                self.u[lo][a] += d;
                self.u[hi][a] -= d;
            }
        }
    }

    /// Dense O(n²) repulsion, one unordered pair at a time (`tmp/d2*scaling_ratio`).
    /// Deviation from the bare reference: an exact coincidence (`d2 == 0`, impossible
    /// on the diagonal but not off it) is nudged apart by the counter hash first, so no
    /// `Infinity`/`NaN` factor can reach a node (D9) — networkx's own dense form has no
    /// such guard.
    fn repulsion(&mut self) {
        let n = self.p.len();
        let k = self.params.scaling_ratio;
        for i in 0..n {
            for j in (i + 1)..n {
                let d = self.repel_delta(i as u32, j as u32);
                let d2 = norm2(&d, self.dim);
                let f = self.mass[i] * self.mass[j] / d2 * k;
                for (a, delta) in d.iter().enumerate().take(self.dim) {
                    self.u[i][a] += delta * f;
                    self.u[j][a] -= delta * f;
                }
            }
        }
    }

    fn repel_delta(&self, i: u32, j: u32) -> [f64; MAX_DIM] {
        let mut d = [0.0; MAX_DIM];
        for (a, slot) in d.iter_mut().enumerate().take(self.dim) {
            *slot = self.p[i as usize][a] - self.p[j as usize][a];
        }
        if norm2(&d, self.dim) == 0.0 {
            for (a, slot) in d.iter_mut().enumerate().take(self.dim) {
                *slot = jiggle(self.params.seed, self.iter_no, a as u32, (i, j));
            }
        }
        d
    }

    /// `gravities = -gravity * mass * unit_vec(pos - mean(pos))`, `strong_gravity=False`.
    fn gravity(&mut self) {
        let n = self.p.len();
        if n == 0 {
            return;
        }
        let dim = self.dim;
        let mut mean = [0.0; MAX_DIM];
        for row in &self.p {
            for a in 0..dim {
                mean[a] += row[a];
            }
        }
        for slot in mean.iter_mut().take(dim) {
            *slot /= n as f64;
        }
        for i in 0..n {
            let mut centred = [0.0; MAX_DIM];
            for (a, slot) in centred.iter_mut().enumerate().take(dim) {
                *slot = self.p[i][a] - mean[a];
            }
            let norm = libm::sqrt(norm2(&centred, dim));
            let g = self.params.gravity * self.mass[i];
            for (a, slot) in self.u[i].iter_mut().enumerate().take(dim) {
                *slot -= if norm > 0.0 {
                    g * centred[a] / norm
                } else {
                    0.0
                };
            }
        }
    }

    fn swing_and_traction(&self) -> (f64, f64) {
        let (mut swing, mut traction) = (0.0, 0.0);
        let dim = self.dim;
        for i in 0..self.p.len() {
            let mut back = [0.0; MAX_DIM];
            let mut fwd = [0.0; MAX_DIM];
            for a in 0..dim {
                back[a] = self.p[i][a] - self.u[i][a];
                fwd[a] = self.p[i][a] + self.u[i][a];
            }
            swing += self.mass[i] * libm::sqrt(norm2(&back, dim));
            traction += 0.5 * self.mass[i] * libm::sqrt(norm2(&fwd, dim));
        }
        (swing, traction)
    }

    /// networkx's own `estimate_factor` helper, verbatim.
    fn estimate_factor(&mut self, swing: f64, traction: f64) {
        let n = self.p.len() as f64;
        let jt = self.params.jitter_tolerance;
        let opt_jitter = 0.05 * libm::sqrt(n);
        let min_jitter = libm::sqrt(opt_jitter);
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
        let dim = self.dim;
        for i in 0..self.p.len() {
            let norm = libm::sqrt(norm2(&self.u[i], dim));
            let factor = self.speed / (1.0 + libm::sqrt(self.speed * self.mass[i] * norm));
            let mut step = 0.0;
            for a in 0..dim {
                let d = self.u[i][a] * factor;
                self.p[i][a] += d;
                step += f64::abs(d);
            }
            moved += step;
        }
        moved
    }
}

/// `sum of squares` over the live axes, ascending — at `dim` 2 this is the `px*px + py*py`
/// the 2D arm always computed, in the same order, so its bits do not move.
fn norm2(v: &[f64; MAX_DIM], dim: usize) -> f64 {
    let mut sum = 0.0;
    for x in v.iter().take(dim) {
        sum += x * x;
    }
    sum
}

/// networkx's own initial positions are `nx.random_layout` (uniform in the unit
/// `dim`-cube, `numpy`'s RNG); this port uses the crate's one sequential generator,
/// [`Mulberry32`], seeded explicitly (devil C8's "two kinds, and only two" — `rng.rs`).
/// Public so the networkx differential can start the reference from the very same
/// positions.
///
/// Row-major, `dim` draws per node in ascending axis order, which at `dim` 2 is the two
/// draws per node the 2D arm always made in that order.
pub fn initial_positions(n: u32, seed: u32, dim: usize) -> Vec<[f64; MAX_DIM]> {
    let mut rng = Mulberry32::new(seed);
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let mut row = [0.0; MAX_DIM];
        for slot in row.iter_mut().take(dim) {
            *slot = rng.next_f64();
        }
        out.push(row);
    }
    out
}

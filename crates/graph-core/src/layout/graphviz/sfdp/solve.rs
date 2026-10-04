//! The spring-electrical iteration at one level: repulsive field, edge attraction, step.
//!
//! Reference: `spring_electrical_embedding` at `lib/sfdpgen/spring_electrical.c:270-375`,
//! read as an algorithm reference. One iteration builds the Barnes-Hut tree, gathers the
//! repulsive force on every node, gathers the attractive force along every edge, normalises
//! each node's total force to unit length, and moves it by `step`.
//!
//! **Gather form (D10), and this is a deviation from the oracle.** [`Solve::relax`] gathers
//! every node's total force from the positions at the *start* of the iteration — [`gather`](Solve::gather)
//! reads `self.x` and writes only a scratch buffer — and only then moves every node, by the
//! step **as it stands at that point** ([`advance`](Solve::advance)). The reference does the opposite: it
//! normalises and moves vertex `i` inside the same loop that computes the next vertex's force
//! (`spring_electrical.c:630-638`), so a node's move can depend on which nodes were moved
//! before it.
//!
//! Before this repair the two were the same code with the opposite comment: the loop updated
//! `self.x[i]` in place, exactly the reference's order, under a doc that claimed gather. The
//! repair made the code match the doc — see `docs/decisions/sfdp-gather-form.md` for why the
//! doc's claim was the thing to keep and what it costs against Graphviz's own output.
//!
//! **Fixed-order reductions (D2).** `Fnorm` is accumulated over nodes in dense index order and
//! nothing iterates a hash map, so the sum is one specific order every run.

use super::force::{self, MAX_ITER, QUADTREE_SIZE, STEP, TOL};
use super::quadtree::Quadtree;
use super::start;
use crate::csr::Csr;

/// The reference's first step size (`spring_electrical.c:58`, `ctrl.step = 0.1`).
pub(super) const FIRST_STEP: f64 = STEP;

/// The seeded random start: Graphviz's `x[i] = drand()` in dense index order
/// (`spring_electrical.c:282-284`), from the glibc-compatible generator in [`start`], and
/// x and y interleaved per node — entry `i` consumes draws `2i` and `2i+1`.
/// The random start from a generator the caller keeps, so that a later step can draw from the
/// same stream: `count` entries, x and y interleaved per node.
///
/// **The driver keeps the generator rather than a seed.** The reference draws its prolongation
/// jitter from this same stream (`spring_electrical.c:1155`) and nothing re-seeds in between, so
/// a port that reseeds per step lands its jitter somewhere else entirely.
pub(super) fn random_start_from(rng: &mut start::Glibc, count: u32) -> (Vec<f64>, Vec<f64>) {
    let positions = rng.positions(count);
    let x = positions.iter().map(|p| p[0]).collect();
    let y = positions.iter().map(|p| p[1]).collect();
    (x, y)
}

/// A solve over one level's graph: the positions, each node's neighbours, the ideal edge
/// length `K`, and whether the step adapts to the force norm.
pub(super) struct Solve {
    pub(super) x: Vec<f64>,
    pub(super) y: Vec<f64>,
    rows: Csr,
    k: f64,
    adaptive: bool,
}

impl Solve {
    /// The coarsest level: `K` is the mean edge length of the start, and the step adapts.
    pub(super) fn new(x: Vec<f64>, y: Vec<f64>, edges: &[(u32, u32)]) -> Self {
        let k = force::average_edge_length(edges, &x, &y);
        Self::with_k(x, y, edges, k).adaptive()
    }

    /// A finer level, at the `K` the driver carried down and without adaptive cooling: the
    /// reference overwrites `ctrl->K` with the decayed value and switches the adaptive step off
    /// below the coarsest level (`spring_electrical.c:1156-1159`).
    pub(super) fn with_k(x: Vec<f64>, y: Vec<f64>, edges: &[(u32, u32)], k: f64) -> Self {
        let rows = neighbours(x.len() as u32, edges);
        Self {
            x,
            y,
            rows,
            k,
            adaptive: false,
        }
    }

    fn adaptive(mut self) -> Self {
        self.adaptive = true;
        self
    }

    /// The ideal edge length this level is solving at.
    pub(super) fn k(&self) -> f64 {
        self.k
    }

    /// Iterate until the step falls to `TOL` or `max_iter` iterations have run
    /// (`spring_electrical.c:567-650`, a do-while: at least one iteration runs).
    pub(super) fn relax(&mut self, step: f64, max_iter: u32) {
        let limit = if max_iter == 0 { MAX_ITER } else { max_iter };
        let (crk, kp) = (force::crk(self.k), force::kp(self.k));
        let mut forces = vec![[0.0f64; 2]; self.x.len()];
        // The reference starts `Fnorm` at 0, so the first iteration always cools.
        let (mut step, mut norm, mut iter) = (step, 0.0, 0u32);
        loop {
            self.gather(&mut forces, kp, crk);
            let previous = norm;
            norm = self.advance(&forces, step);
            step = force::update_step(self.adaptive, step, norm, previous);
            iter += 1;
            if step <= TOL || iter >= limit {
                return;
            }
        }
    }

    /// Every node's total force from the positions at the start of the iteration: attraction
    /// along its edges, then repulsion, exact below [`QUADTREE_SIZE`] nodes and through the
    /// quadtree above it (`spring_electrical.c:543`).
    fn gather(&self, forces: &mut [[f64; 2]], kp: f64, crk: f64) {
        let tree = (self.x.len() >= QUADTREE_SIZE).then(|| Quadtree::of(&self.x, &self.y));
        for (i, slot) in (0u32..).zip(forces.iter_mut()) {
            let mut out = [0.0f64; 2];
            for &j in self.rows.row(i) {
                force::attract(&mut out, i, j, &self.x, &self.y, crk);
            }
            let push = match &tree {
                Some(tree) => tree.repulsion(i, kp),
                None => self.all_pairs(i as usize, kp),
            };
            *slot = [out[0] + push[0], out[1] + push[1]];
        }
    }

    /// The exact repulsion on node `i` from every other node.
    fn all_pairs(&self, i: usize, kp: f64) -> [f64; 2] {
        let mut out = [0.0f64; 2];
        for j in (0..self.x.len()).filter(|&j| j != i) {
            force::repel(&mut out, [self.x[i] - self.x[j], self.y[i] - self.y[j]], kp);
        }
        out
    }

    /// Moves every node by `step` along its unit force, returning the sum of the force lengths
    /// (`Fnorm`), accumulated in dense index order (D2).
    fn advance(&mut self, forces: &[[f64; 2]], step: f64) -> f64 {
        let mut norm = 0.0;
        for (i, f) in forces.iter().enumerate() {
            let length = f64::sqrt(f[0] * f[0] + f[1] * f[1]);
            norm += length;
            if length > 0.0 {
                self.x[i] += step * (f[0] / length);
                self.y[i] += step * (f[1] / length);
            }
        }
        norm
    }
}

/// Each node's neighbours, both directions of every edge, self loops dropped (the reference's
/// `if (ja[j] == i) continue`).
fn neighbours(count: u32, edges: &[(u32, u32)]) -> Csr {
    let pairs = edges
        .iter()
        .filter(|&&(a, b)| a != b)
        .flat_map(|&(a, b)| [(a, b), (b, a)]);
    Csr::from_pairs(count, pairs).expect("an edge count fits u32")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The three-node path: the solve must terminate, stay finite, and separate the nodes.
    #[test]
    fn relaxing_a_path_terminates_and_separates_the_nodes() {
        let edges = [(0u32, 1u32), (1, 2)];
        let mut solve = Solve::new(vec![0.1, 0.5, 0.9], vec![0.5, 0.5, 0.5], &edges);
        solve.relax(0.1, 200);
        assert!(solve.x.iter().chain(solve.y.iter()).all(|v| v.is_finite()));
        let span = solve.x[2] - solve.x[0];
        assert!(span > 0.0, "the path collapsed: {:?}", solve.x);
    }

    /// Two runs of one input agree bit-for-bit. Without this, the gather in `force_on` could
    /// be reading positions it has already moved.
    #[test]
    fn two_relaxations_are_bit_identical() {
        let edges = [(0u32, 1u32), (1, 2), (2, 0)];
        let run = || {
            let mut solve = Solve::new(vec![0.1, 0.5, 0.9], vec![0.5, 0.2, 0.7], &edges);
            solve.relax(0.1, 50);
            (solve.x, solve.y)
        };
        let (x1, y1) = run();
        let (x2, y2) = run();
        for (a, b) in x1.iter().zip(&x2) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        for (a, b) in y1.iter().zip(&y2) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    /// The ideal edge length is the mean edge length of the positions it was given, so a
    /// driver that reuses a stale `K` after moving the points lays out at the wrong scale.
    #[test]
    fn k_is_the_mean_edge_length_of_the_positions_it_is_given() {
        let edges = [(0u32, 1u32)];
        let solve = Solve::new(vec![0.0, 4.0], vec![0.0, 0.0], &edges);
        assert!((solve.k() - 4.0).abs() < 1e-12, "k was {}", solve.k());
    }

    /// An edgeless graph still relaxes without dividing by anything: `K` defaults to 1 and the
    /// repulsion alone is a valid, finite move.
    #[test]
    fn an_edgeless_graph_relaxes_finitely() {
        let mut solve = Solve::new(vec![0.5, 0.25], vec![0.5, 0.75], &[]);
        solve.relax(0.1, 20);
        assert!(solve.x.iter().chain(solve.y.iter()).all(|v| v.is_finite()));
        assert_eq!(solve.k(), 1.0);
    }

    /// Coincident nodes are the pathological input: the depth cap in the quadtree and the
    /// `length > 0.0` guard here are what keep it finite.
    #[test]
    fn coincident_nodes_relax_finitely() {
        let edges = [(0u32, 1u32), (1, 2)];
        let mut solve = Solve::new(vec![0.5, 0.5, 0.5], vec![0.5, 0.5, 0.5], &edges);
        solve.relax(0.1, 30);
        assert!(solve.x.iter().chain(solve.y.iter()).all(|v| v.is_finite()));
    }

    #[test]
    fn a_self_loop_contributes_no_force() {
        let gathered = |edges: &[(u32, u32)]| {
            let solve = Solve::new(vec![0.0, 1.0], vec![0.0, 0.0], edges);
            let mut forces = vec![[0.0; 2]; 2];
            solve.gather(&mut forces, 1.0, 1.0);
            forces
        };
        assert_eq!(
            gathered(&[(0, 0)]),
            gathered(&[]),
            "a self loop moved node 0"
        );
    }
}

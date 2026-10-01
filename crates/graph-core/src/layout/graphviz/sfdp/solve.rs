//! The spring-electrical iteration at one level: repulsive field, edge attraction, step.
//!
//! Reference: `spring_electrical_embedding` at `lib/sfdpgen/spring_electrical.c:270-375`,
//! read as an algorithm reference. One iteration builds the Barnes-Hut tree, gathers the
//! repulsive force on every node, gathers the attractive force along every edge, normalises
//! each node's total force to unit length, and moves it by `step`.
//!
//! **Gather form (D10).** Node `i`'s displacement is computed entirely from the positions at
//! the *start* of the iteration, so the result does not depend on the order nodes are updated
//! in, and is bit-identical native vs wasm32. The reference moves each vertex as it goes,
//! which makes its result sequential in the vertex loop; the same model with the dependency
//! removed is what runs here.
//!
//! **Fixed-order reductions (D2).** `norm` is accumulated over nodes in dense index order and
//! nothing iterates a hash map, so the sum is one specific order every run.

use super::force::{self, MAX_ITER, STEP, TOL};
use super::quadtree::Quadtree;
use super::start;

/// The reference's first step size (`spring_electrical.c:58`, `ctrl.step = 0.1`).
pub(super) const FIRST_STEP: f64 = STEP;

/// The seeded random start: Graphviz's `x[i] = drand()` in dense index order
/// (`spring_electrical.c:282-284`), from the glibc-compatible generator in [`start`].
///
/// The x and y columns come out interleaved per node, so entry `i` consumes draws `2i` and
/// `2i+1` of the stream — gather form (D10), and the reason a permutation of the node order
/// would give a different drawing.
pub(super) fn random_start(count: u32, seed: u32) -> (Vec<f64>, Vec<f64>) {
    let positions = start::start_positions(count, seed);
    let x = positions.iter().map(|p| p[0]).collect();
    let y = positions.iter().map(|p| p[1]).collect();
    (x, y)
}

/// A solve over one level's graph: the positions, the edges, the ideal edge length `K`, and
/// the force norm the last iteration ended on.
pub(super) struct Solve<'a> {
    pub(super) x: Vec<f64>,
    pub(super) y: Vec<f64>,
    pub(super) edges: &'a [(u32, u32)],
    k: f64,
    norm: f64,
}

impl<'a> Solve<'a> {
    /// A solve over `edges`, positioned at `x`/`y`, with `K` taken from the mean edge length.
    pub(super) fn new(x: Vec<f64>, y: Vec<f64>, edges: &'a [(u32, u32)]) -> Self {
        let k = force::average_edge_length(edges, &x, &y);
        Self {
            x,
            y,
            edges,
            k,
            norm: f64::MAX,
        }
    }

    /// A solve at an explicit `K`, which is what every level below the coarsest uses: the
    /// driver carries the decayed `K` down rather than recomputing the mean at each level,
    /// because the reference overwrites `ctrl->K` with the decayed value instead
    /// (`spring_electrical.c:1159`).
    pub(super) fn with_k(x: Vec<f64>, y: Vec<f64>, edges: &'a [(u32, u32)], k: f64) -> Self {
        Self {
            x,
            y,
            edges,
            k,
            norm: f64::MAX,
        }
    }

    /// The ideal edge length this level is solving at.
    pub(super) fn k(&self) -> f64 {
        self.k
    }

    /// Iterate until the step falls below `TOL / K` or `max_iter` is reached.
    pub(super) fn relax(&mut self, step: f64, max_iter: u32) {
        let limit = if max_iter == 0 { MAX_ITER } else { max_iter };
        let crk = force::crk(self.k);
        let kp = force::kp(self.k);
        // The first iteration has no previous norm to compare against; the reference seeds
        // `Fnorm0` with 0, which makes the first step always cool, and so does this.
        let mut current = step;
        let mut iter = 0u32;
        while current > TOL / self.k && iter < limit {
            let previous = if iter == 0 { 0.0 } else { self.norm };
            self.norm = 0.0;
            let tree = Quadtree::of(&self.x, &self.y);
            let count = self.x.len();
            for i in 0..count {
                let force = self.force_on(&tree, i as u32, kp, crk);
                let length = libm::sqrt(force[0] * force[0] + force[1] * force[1]);
                self.norm += length;
                if length > 0.0 {
                    let scale = step / length;
                    self.x[i] += force[0] * scale;
                    self.y[i] += force[1] * scale;
                }
            }
            current = force::update_step(current, self.norm, previous);
            iter += 1;
        }
    }

    /// The gathered total force on node `i`: repulsion from the tree, then attraction along
    /// every edge `i` is an endpoint of.
    fn force_on(&self, tree: &Quadtree, i: u32, kp: f64, crk: f64) -> [f64; 2] {
        let mut out = [0.0f64; 2];
        tree.repulsion(&mut out, &self.x, &self.y, i as usize, kp);
        for &(a, b) in self.edges {
            // The spring pulls `i` toward the *other* end. A self loop contributes nothing, and
            // is skipped rather than adding a zero, which is what the reference's
            // `if (ja[j] == i) continue` does.
            match (a == i, b == i) {
                (true, true) => continue,
                (true, false) => force::attract(&mut out, i, b, &self.x, &self.y, crk),
                (false, true) => force::attract(&mut out, i, a, &self.x, &self.y, crk),
                (false, false) => continue,
            }
        }
        out
    }
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
        let with = Solve::new(vec![0.0, 1.0], vec![0.0, 0.0], &[(0u32, 0u32)]);
        let without = Solve::new(vec![0.0, 1.0], vec![0.0, 0.0], &[]);
        let tree = Quadtree::of(&with.x, &with.y);
        let a = with.force_on(&tree, 0, 1.0, 1.0);
        let tree = Quadtree::of(&without.x, &without.y);
        let b = without.force_on(&tree, 0, 1.0, 1.0);
        assert_eq!(
            [a[0].to_bits(), a[1].to_bits()],
            [b[0].to_bits(), b[1].to_bits()],
            "a self loop moved node 0"
        );
    }
}

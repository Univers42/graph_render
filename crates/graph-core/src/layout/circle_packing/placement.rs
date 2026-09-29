//! Places every packed circle's centre by walking the triangulation's dual graph
//! (`_lay_out_packing`, `circle_packing.py:179-241`), then relaxes the walk's rounding
//! drift back toward exact tangency (`_refine_tangency`, `circle_packing.py:243-279`).
//! `f64` throughout; positions are plain `(x, y)` pairs, not `complex`, since Rust has no
//! native complex type worth pulling a dependency in for two fields.

use super::radii::packing_angle;
use core::f64::consts::PI;
use indexmap::{IndexMap, IndexSet};
use std::collections::VecDeque;

/// Every triangulated vertex's centre, walked from `triangles[0]`'s exact tangent seed,
/// and which vertices the walk reached. `false` entries should not occur once
/// [`crate::layout::planarity::triangulate_embedding`] has joined every component into
/// one triangulated disk; the caller falls back rather than trust an unreached node
/// (module Ponytail, `super`'s doc).
pub(super) fn lay_out_packing(
    triangles: &[[u32; 3]],
    radii: &[f64],
    n: u32,
) -> (Vec<(f64, f64)>, Vec<bool>) {
    let map = half_edge_map(triangles);
    let mut walk = Walk::new(n);
    let [a0, b0, c0] = triangles[0];
    seed_triangle(&mut walk, radii, triangles[0]);
    let mut queue = VecDeque::from([(b0, a0), (c0, b0), (a0, c0)]);
    let mut done: IndexSet<(u32, u32)> = IndexSet::new();
    while let Some((a, b)) = queue.pop_front() {
        if !done.insert((a, b)) {
            continue;
        }
        let Some(&c) = map.get(&(a, b)) else {
            continue;
        };
        if !walk.is_placed(c) {
            walk.try_place_third(radii, a, b, c);
        }
        if walk.is_placed(c) {
            queue.push_back((c, b));
            queue.push_back((a, c));
        }
    }
    (walk.centres, walk.placed)
}

/// The opening triangle's three centres (`circle_packing.py:192-197`): `a0` at the
/// origin, `b0` tangent along the positive x axis, `c0` at the exact angle the three
/// radii dictate.
fn seed_triangle(walk: &mut Walk, radii: &[f64], [a0, b0, c0]: [u32; 3]) {
    walk.set(a0, (0.0, 0.0));
    walk.set(b0, (radii[a0 as usize] + radii[b0 as usize], 0.0));
    let angle = packing_angle(radii[a0 as usize], radii[b0 as usize], radii[c0 as usize]);
    walk.set(c0, polar(radii[a0 as usize] + radii[c0 as usize], angle));
}

fn polar(r: f64, theta: f64) -> (f64, f64) {
    (r * libm::cos(theta), r * libm::sin(theta))
}

/// `w` lies left of `u -> v` in every triangle, so `(u, v) -> w` finds the next circle to
/// place from any already-placed edge (`half_edge_map`, `circle_packing.py:182-188`). An
/// `IndexMap` keeps this at `O(triangles)` memory — a dense `n * n` table would not.
fn half_edge_map(triangles: &[[u32; 3]]) -> IndexMap<(u32, u32), u32> {
    let mut map = IndexMap::with_capacity(triangles.len() * 3);
    for &[a, b, c] in triangles {
        for (u, v, w) in [(a, b, c), (b, c, a), (c, a, b)] {
            map.insert((u, v), w);
        }
    }
    map
}

/// The centres placed so far, and which vertices have one.
struct Walk {
    centres: Vec<(f64, f64)>,
    placed: Vec<bool>,
}

impl Walk {
    fn new(n: u32) -> Self {
        Self {
            centres: vec![(0.0, 0.0); n as usize],
            placed: vec![false; n as usize],
        }
    }

    fn set(&mut self, v: u32, pos: (f64, f64)) {
        self.centres[v as usize] = pos;
        self.placed[v as usize] = true;
    }

    fn is_placed(&self, v: u32) -> bool {
        self.placed[v as usize]
    }

    /// Places `c` tangent to already-placed `a` and `b` (`circle_packing.py:213-231`):
    /// the angle at `a` between `b` and `c`, from the *measured* `a`-`b` distance so
    /// rounding drift never breaks closure. Leaves `c` unplaced when `a` and `b`
    /// coincide (should not happen once seeded from a non-degenerate triangle).
    fn try_place_third(&mut self, radii: &[f64], a: u32, b: u32, c: u32) {
        let (pa, pb) = (self.centres[a as usize], self.centres[b as usize]);
        let d_ac = radii[a as usize] + radii[c as usize];
        let d_bc = radii[b as usize] + radii[c as usize];
        let vec_ab = (pb.0 - pa.0, pb.1 - pa.1);
        let dist_ab = libm::hypot(vec_ab.0, vec_ab.1);
        if dist_ab <= 1e-12 {
            return;
        }
        let denom = 2.0 * d_ac * dist_ab;
        let theta = if denom < 1e-12 {
            PI / 3.0
        } else {
            let cos_val =
                ((d_ac * d_ac + dist_ab * dist_ab - d_bc * d_bc) / denom).clamp(-1.0, 1.0);
            libm::acos(cos_val)
        };
        let phase = libm::atan2(vec_ab.1, vec_ab.0) + theta;
        let offset = polar(d_ac, phase);
        self.set(c, (pa.0 + offset.0, pa.1 + offset.1));
    }
}

/// Gradient descent pulling `edges` back to tangency, damped by the whole step's own
/// norm so one bad node cannot dominate (`_refine_tangency`, `circle_packing.py:243-279`).
/// Runs on the triangulation's edges, not the graph's own: chasing the graph's edges
/// alone would let the packing pull apart along the chords the triangulation added.
pub(super) fn refine_tangency(
    mut positions: Vec<(f64, f64)>,
    radii: &[f64],
    edges: &[(u32, u32)],
    iterations: u32,
) -> Vec<(f64, f64)> {
    if edges.is_empty() {
        return positions;
    }
    let target: Vec<f64> = edges
        .iter()
        .map(|&(u, v)| radii[u as usize] + radii[v as usize])
        .collect();
    let incidence = incidence_lists(edges, positions.len());
    for _ in 0..iterations {
        let (pulls, worst) = edge_pulls(&positions, edges, &target);
        if worst < 1e-9 {
            break;
        }
        let gradients = gather_gradients(&pulls, &incidence, edges);
        if !apply_gradients(&mut positions, &gradients) {
            break;
        }
    }
    positions
}

/// `incidence[i]`: every index into `edges` incident to node `i`, ascending — the order
/// the edge loop visited them in, which is the order the gather below must sum them in
/// to stay bit-identical. A self-loop (which `tri_edges` never emits, but a caller may)
/// appears twice, so its `+` then `-` on the same node survives the rewrite.
fn incidence_lists(edges: &[(u32, u32)], n: usize) -> Vec<Vec<usize>> {
    let mut incidence = vec![Vec::new(); n];
    for (i, &(u, v)) in edges.iter().enumerate() {
        incidence[u as usize].push(i);
        incidence[v as usize].push(i);
    }
    incidence
}

/// Every edge's own pull, from the round's start-of-step positions, in edge order, plus
/// the worst error among them. `pulls[e]` is the term added into edge `e`'s `u` endpoint
/// (and negated into its `v`); a pair too close to have a direction contributes nothing,
/// as the reference's `alive` mask does (`circle_packing.py:255-266`).
fn edge_pulls(
    positions: &[(f64, f64)],
    edges: &[(u32, u32)],
    target: &[f64],
) -> (Vec<Option<(f64, f64)>>, f64) {
    let mut pulls = Vec::with_capacity(edges.len());
    let mut worst = 0.0_f64;
    for (i, &edge) in edges.iter().enumerate() {
        match edge_pull(positions, edge, target[i]) {
            Some((pull, error)) => {
                worst = worst.max(error);
                pulls.push(Some(pull));
            }
            None => pulls.push(None),
        }
    }
    (pulls, worst)
}

/// One edge's pull toward `target` and the error behind it; `None` for a pair too close
/// to have a direction, the same guard as the placement walk.
fn edge_pull(positions: &[(f64, f64)], edge: (u32, u32), target: f64) -> Option<((f64, f64), f64)> {
    let (u, v) = edge;
    let (pu, pv) = (positions[u as usize], positions[v as usize]);
    let diff = (pu.0 - pv.0, pu.1 - pv.1);
    let dist = libm::hypot(diff.0, diff.1);
    if dist <= 1e-12 {
        return None;
    }
    let error = dist - target;
    Some(((error / dist * diff.0, error / dist * diff.1), error.abs()))
}

/// Each node's gradient, summed from its own incident edges in [`incidence_lists`]
/// order — the gather D10 requires: element `i` reads only start-of-step state and
/// writes only `out[i]`, and `v`'s term is the negation of the very same product `u`'s
/// is, so this is bit-identical to the `gradients[u] += / gradients[v] -=` scatter it
/// replaces (proved bit for bit by `tests::the_gather_is_bit_identical_to_the_scatter_over_seeded_random_cases`).
fn gather_gradients(
    pulls: &[Option<(f64, f64)>],
    incidence: &[Vec<usize>],
    edges: &[(u32, u32)],
) -> Vec<(f64, f64)> {
    let mut gradients = vec![(0.0, 0.0); incidence.len()];
    for (i, incident) in incidence.iter().enumerate() {
        let mut gx = 0.0;
        let mut gy = 0.0;
        for &e in incident {
            let Some((sx, sy)) = pulls[e] else { continue };
            if edges[e].0 as usize == i {
                gx += sx;
                gy += sy;
            } else {
                gx -= sx;
                gy -= sy;
            }
        }
        gradients[i] = (gx, gy);
    }
    gradients
}

const LEARNING_RATE: f64 = 0.1;

/// One damped step along `gradients`; `false` (stop) once the whole step's norm is
/// negligible.
fn apply_gradients(positions: &mut [(f64, f64)], gradients: &[(f64, f64)]) -> bool {
    let grad_norm = libm::sqrt(gradients.iter().map(|g| g.0 * g.0 + g.1 * g.1).sum::<f64>());
    if grad_norm < 1e-12 {
        return false;
    }
    let scale = LEARNING_RATE / (1.0 + 0.1 * grad_norm);
    for (p, g) in positions.iter_mut().zip(gradients) {
        p.0 -= scale * g.0;
        p.1 -= scale * g.1;
    }
    true
}

#[cfg(test)]
mod tests;

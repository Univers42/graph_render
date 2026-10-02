//! The Kamada-Kawai Newton descent at a const number of dimensions, from the prose spec
//! `docs/layouts/layout.force.kamada_kawai.md` and the paper (Kamada and Kawai, Information
//! Processing Letters 31(1), 1989) alone (`docs/decisions/layouts-igraph.md` rule 1: spec and
//! papers, never igraph's sources).
//!
//! **One kernel at `D` columns, not one kernel per dimension.** `D` is a const parameter, and
//! at `D = 2` the generalised Cramer's rule below is the closed form the spec names for 2D:
//! the determinant of the symmetric 2x2 block expands to `a * c - b * b` and the two numerators
//! to `c * g0 - b * g1` and `a * g1 - b * g0`, in that order. `layout.force.kamada_kawai` is
//! this file at `D = 2` and `layout.force.kamada_kawai_3d` at `D = 3`, the dimension SciGraphs
//! actually calls (`igraph_layouts.py:99`).
//!
//! **The start layout is the one thing the kernel does not own.** The spec gives a circle for
//! 2D and a sphere for 3D, and they are different closed forms in different modules, so each
//! dimension's stage hands its own start in. Everything after it — distances, spring constants,
//! the gradient, the block, the descent — is shared and dimension-independent.
//!
//! **Gather form (D10).** Every output of an iteration is `g_m`, `H_m` and the new `g_i`, all
//! read from the start-of-iteration positions; the position update touches `p_m` alone. Two
//! threads never see each other's writes, and the snapshot is bit-identical native vs wasm32.
//!
//! **Zero distance.** Two coincident vertices make `delta / |delta|` undefined, and the spec
//! records that igraph does not guard it. Here `r == 0` contributes the pure stretch term only
//! (`l` direction undefined), and a singular or non-finite block contributes a zero step. That
//! guard is what keeps the 3-D descent finite on the three-node path where igraph's 3x3 Newton
//! block overflows — see `docs/measurements/scigraphs-conformance.md`, the `IGRAPH_KK` row.

use super::SimpleGraph;
use super::kamada_kawai::KkParams;

/// Gradients below this norm are treated as equilibrium: the step is zero. The spec's
/// `KK_EPS`.
pub(super) const KK_EPS: f64 = 1e-13;

/// Newton descent on one vertex at a time from `start`, the dimension's own start layout.
pub(super) fn descend<const D: usize>(
    graph: &SimpleGraph,
    n: usize,
    mut pos: Vec<[f64; D]>,
    params: &KkParams,
) -> Vec<[f64; D]> {
    if n > 1 {
        let springs = Springs::new(graph, n, params.kkconst.unwrap_or(n as f64));
        walk(&mut pos, &springs, params);
    }
    pos
}

/// Hop distances and the two per-pair constants derived from them.
struct Springs {
    n: usize,
    dist: Vec<f64>,
    length_per_hop: f64,
    strength: f64,
}

impl Springs {
    /// `l_ij = sqrt(n) / d_max * d_ij` and `k_ij = K / d_ij^2`. An unreachable distance, and
    /// a zero one, both become `d_max`, so a disconnected graph's components are `d_max` apart.
    /// Ponytail: an edgeless graph has no finite `d_max` and igraph divides by zero there; this
    /// port takes every distance as 1 and draws a regular-polygon-like cloud instead.
    fn new(graph: &SimpleGraph, n: usize, strength: f64) -> Self {
        let mut dist = all_pairs_hops(graph, n);
        let finite_max = dist
            .iter()
            .copied()
            .filter(|d| d.is_finite())
            .fold(0.0, f64::max);
        let d_max = if finite_max > 0.0 { finite_max } else { 1.0 };
        for d in dist.iter_mut().filter(|d| !d.is_finite() || **d == 0.0) {
            *d = d_max;
        }
        Springs {
            n,
            dist,
            length_per_hop: libm::sqrt(n as f64) / d_max,
            strength,
        }
    }

    /// Spring `(k, l)` between `i` and `j`.
    fn spring(&self, i: usize, j: usize) -> (f64, f64) {
        let d = self.dist[i * self.n + j];
        (self.strength / (d * d), self.length_per_hop * d)
    }
}

/// Breadth-first hop counts from every vertex; unreachable is infinity. Ascending source,
/// ascending queue order, so the matrix is a function of the edge order alone (D3).
fn all_pairs_hops(graph: &SimpleGraph, n: usize) -> Vec<f64> {
    let mut dist = vec![f64::INFINITY; n * n];
    let mut queue = Vec::with_capacity(n);
    for s in 0..n {
        queue.clear();
        queue.push(s);
        dist[s * n + s] = 0.0;
        let mut head = 0;
        while head < queue.len() {
            let v = queue[head];
            head += 1;
            for &e in graph.rows.row(v as u32) {
                let e = e as usize;
                let u = if graph.lo[e] as usize == v {
                    graph.hi[e]
                } else {
                    graph.lo[e]
                } as usize;
                if dist[s * n + u].is_infinite() {
                    dist[s * n + u] = dist[s * n + v] + 1.0;
                    queue.push(u);
                }
            }
        }
    }
    dist
}

/// At most `maxiter` single-vertex moves: pick the worst gradient, take its Newton step, then
/// refresh every gradient incrementally. `epsilon` stops early when the largest squared
/// gradient norm falls below it; the spec's default of 0 never does.
fn walk<const D: usize>(pos: &mut [[f64; D]], springs: &Springs, params: &KkParams) {
    let n = springs.n;
    let mut grad: Vec<[f64; D]> = (0..n).map(|m| full_gradient(pos, springs, m)).collect();
    let maxiter = params.maxiter.unwrap_or(50 * n as u32);
    for _ in 0..maxiter {
        let (m, worst) = pick(&grad);
        if worst < params.epsilon {
            break;
        }
        let step = newton_step(pos, springs, m, grad[m]);
        for i in (0..n).filter(|&i| i != m) {
            let old = pull(pos, springs, i, m);
            for axis in 0..D {
                grad[i][axis] -= old[axis];
            }
        }
        for (axis, slot) in pos[m].iter_mut().enumerate() {
            *slot -= step[axis];
        }
        for i in (0..n).filter(|&i| i != m) {
            let new = pull(pos, springs, i, m);
            for axis in 0..D {
                grad[i][axis] += new[axis];
            }
        }
        grad[m] = full_gradient(pos, springs, m);
    }
}

/// The vertex with the largest squared gradient norm, ties to the lowest index (only a
/// strictly greater norm replaces the incumbent), and that norm.
fn pick<const D: usize>(grad: &[[f64; D]]) -> (usize, f64) {
    grad.iter().enumerate().fold((0, -1.0), |best, (i, g)| {
        let norm = squared(g);
        if norm > best.1 { (i, norm) } else { best }
    })
}

/// Gradient of `E` on `m` alone, summed over the other vertices in ascending index (D3).
fn full_gradient<const D: usize>(pos: &[[f64; D]], springs: &Springs, m: usize) -> [f64; D] {
    let mut g = [0.0; D];
    for i in (0..springs.n).filter(|&i| i != m) {
        let p = pull(pos, springs, m, i);
        for axis in 0..D {
            g[axis] += p[axis];
        }
    }
    g
}

/// Gradient contribution on `m` of its spring to `i`: `k * (delta - l * delta / |delta|)`.
/// A coincident pair contributes its pure stretch term only, the direction being undefined.
fn pull<const D: usize>(pos: &[[f64; D]], springs: &Springs, m: usize, i: usize) -> [f64; D] {
    let (k, l) = springs.spring(m, i);
    let delta = separation(&pos[m], &pos[i]);
    let r = libm::sqrt(squared(&delta));
    let shrink = if r > 0.0 { l / r } else { 0.0 };
    let mut p = [0.0; D];
    for (axis, d) in delta.iter().enumerate() {
        p[axis] = k * (d - shrink * d);
    }
    p
}

/// `pos[m] - pos[i]`, axis by axis in axis order.
fn separation<const D: usize>(m: &[f64; D], i: &[f64; D]) -> [f64; D] {
    let mut delta = [0.0; D];
    for (axis, slot) in delta.iter_mut().enumerate() {
        *slot = m[axis] - i[axis];
    }
    delta
}

/// The Newton step `H^-1 g` for vertex `m` alone; zero at equilibrium, and zero on a singular
/// or non-finite block. The sign convention is the spec's: `H * step = g`, and the caller
/// subtracts the step.
fn newton_step<const D: usize>(
    pos: &[[f64; D]],
    springs: &Springs,
    m: usize,
    g: [f64; D],
) -> [f64; D] {
    if squared(&g) < KK_EPS * KK_EPS {
        return [0.0; D];
    }
    let h = block(pos, springs, m);
    let det = determinant(&h, None, &[0.0; D]);
    if det == 0.0 || !det.is_finite() {
        return [0.0; D];
    }
    let mut step = [0.0; D];
    for (axis, slot) in step.iter_mut().enumerate() {
        *slot = determinant(&h, Some(axis), &g) / det;
    }
    step
}

/// The Hessian block of `E` for `m` alone, the other vertices held fixed: diagonal
/// `sum k (1 - l * (other axes squared) / r^3)`, off-diagonal `sum k l delta_a delta_b / r^3`.
fn block<const D: usize>(pos: &[[f64; D]], springs: &Springs, m: usize) -> [[f64; D]; D] {
    let mut h = [[0.0; D]; D];
    for i in (0..springs.n).filter(|&i| i != m) {
        let (k, l) = springs.spring(m, i);
        let delta = separation(&pos[m], &pos[i]);
        let r = libm::sqrt(squared(&delta));
        if r == 0.0 {
            for (axis, diagonal) in h.iter_mut().enumerate() {
                diagonal[axis] += k;
            }
            continue;
        }
        let r3 = r * r * r;
        for (a, diagonal) in h.iter_mut().enumerate() {
            let mut rest = 0.0;
            for (b, d) in delta.iter().enumerate() {
                if b != a {
                    rest += l * d * d / r3;
                }
            }
            diagonal[a] += k * (1.0 - rest);
        }
        for a in 0..D {
            for (b, d) in delta.iter().enumerate().skip(a + 1) {
                let off = k * l * delta[a] * d / r3;
                h[a][b] += off;
                h[b][a] += off;
            }
        }
    }
    h
}

/// Determinant of `h` with column `col` replaced by `rhs`, by signed permutation expansion in
/// lexicographic order (D3). `col` of `None` leaves the block alone, which is the determinant
/// itself rather than a Cramer numerator.
fn determinant<const D: usize>(h: &[[f64; D]; D], col: Option<usize>, rhs: &[f64; D]) -> f64 {
    let mut total = 0.0;
    each_permutation::<D>(|p, sign| {
        let mut product = sign;
        for (k, &row) in p.iter().enumerate() {
            product *= if Some(k) == col { rhs[row] } else { h[row][k] };
        }
        total += product;
    });
    total
}

/// Visits every permutation of `0..D` in lexicographic order with its sign. `D` is 2 or 3, so
/// this is a handful of swaps; the order is what fixes the summation order (D3).
fn each_permutation<const D: usize>(mut visit: impl FnMut([usize; D], f64)) {
    let mut p: [usize; D] = core::array::from_fn(|i| i);
    loop {
        visit(p, sign_of(&p));
        if !advance(&mut p) {
            return;
        }
    }
}

/// One lexicographic step over the permutations of `0..D`: the pivot is the largest index
/// whose successor is larger, its partner the smallest index above it holding something
/// larger, and the tail above the pivot reverses. `false` once every permutation has been
/// visited — which is when no such pivot exists, the array being in descending order.
///
/// The descending-tail test is what a wrong pivot rule gets wrong quietly: an inverted rule
/// finds no pivot in the identity either, and a one-permutation walk drops half of every
/// 2x2 determinant without failing a shape check. `tests::the_permutation_walk_visits_each_one_once_and_stops`
/// is the control.
fn advance<const D: usize>(p: &mut [usize; D]) -> bool {
    let Some(pivot) = (0..D.saturating_sub(1)).rev().find(|&i| p[i] < p[i + 1]) else {
        return false;
    };
    let Some(swap) = (pivot + 1..D).rev().find(|&j| p[j] > p[pivot]) else {
        return false;
    };
    p.swap(pivot, swap);
    p[(pivot + 1)..].reverse();
    true
}

/// `+1` for an even permutation, `-1` for an odd one, by inversion count.
fn sign_of<const D: usize>(p: &[usize; D]) -> f64 {
    let mut inversions = 0;
    for a in 0..D {
        for b in (a + 1)..D {
            if p[a] > p[b] {
                inversions += 1;
            }
        }
    }
    if inversions % 2 == 0 { 1.0 } else { -1.0 }
}

/// The squared length over the `D` columns, finished inside the axis loop in axis order. For
/// `D = 2` the leading `0.0 + dx * dx` is `dx * dx`: no product of two `f64`s is a negative
/// zero, so the leading zero cannot move a bit.
fn squared<const D: usize>(delta: &[f64; D]) -> f64 {
    let mut sum = 0.0;
    for &v in delta {
        sum += v * v;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visited<const D: usize>() -> Vec<[usize; D]> {
        let mut seen = Vec::new();
        each_permutation::<D>(|p, _| seen.push(p));
        seen
    }

    #[test]
    fn the_permutation_walk_visits_each_one_once_and_stops() {
        let two = visited::<2>();
        assert_eq!(two, vec![[0, 1], [1, 0]], "2! permutations, both of them");
        let three = visited::<3>();
        assert_eq!(three.len(), 6, "3! permutations");
        for (i, a) in three.iter().enumerate() {
            assert!(three[i + 1..].iter().all(|b| b != a), "a repeat: {a:?}");
        }
    }

    /// The control for the failure the pivot rule above invites: a walk that visits only the
    /// identity yields `a * c`, which is a plausible-looking number and not the determinant.
    #[test]
    fn a_walk_that_dropped_the_second_permutation_would_fail() {
        let h = [[2.0, 1.0], [1.0, 3.0]];
        let full = determinant(&h, None, &[0.0; 2]);
        let identity_only = h[0][0] * h[1][1];
        assert_ne!(full, identity_only);
        assert_eq!(full, 2.0 * 3.0 - 1.0 * 1.0);
    }

    #[test]
    fn the_walk_is_lexicographic() {
        let three = visited::<3>();
        let mut sorted = three.clone();
        sorted.sort();
        assert_eq!(three, sorted, "the order the summation runs in");
    }

    #[test]
    fn the_2x2_determinant_is_the_closed_form() {
        // [[a, b], [b, c]] expands to a * c - b * b, and the Cramer numerator for the first
        // unknown to c * g0 - b * g1 — the two expressions the 2-D stage was pinned to.
        let h = [[2.0, 1.0], [1.0, 3.0]];
        assert_eq!(determinant(&h, None, &[0.0; 2]), 2.0 * 3.0 - 1.0 * 1.0);
        let g = [5.0, 7.0];
        assert_eq!(determinant(&h, Some(0), &g), 3.0 * 5.0 - 1.0 * 7.0);
        assert_eq!(determinant(&h, Some(1), &g), 2.0 * 7.0 - 5.0 * 1.0);
    }

    #[test]
    fn the_3x3_determinant_is_the_closed_form() {
        let h = [[6.0, 1.0, 2.0], [1.0, 5.0, 3.0], [2.0, 3.0, 4.0]];
        let want = 6.0 * (5.0 * 4.0 - 3.0 * 3.0) - 1.0 * (1.0 * 4.0 - 3.0 * 2.0)
            + 2.0 * (1.0 * 3.0 - 5.0 * 2.0);
        assert!(
            (determinant(&h, None, &[0.0; 3]) - want).abs() < 1e-12,
            "{want}"
        );
    }
}

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

use super::kamada_kawai::KkParams;
use super::SimpleGraph;

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
        if norm > best.1 {
            (i, norm)
        } else {
            best
        }
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
    for axis in 0..D {
        step[axis] = determinant(&h, Some(axis), &g) / det;
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
            for axis in 0..D {
                h[axis][axis] += k;
            }
            continue;
        }
        let r3 = r * r * r;
        for a in 0..D {
            let mut rest = 0.0;
            for b in 0..D {
                if b != a {
                    rest += l * delta[b] * delta[b] / r3;
                }
            }
            h[a][a] += k * (1.0 - rest);
        }
        for a in 0..D {
            for b in (a + 1)..D {
                let off = k * l * delta[a] * delta[b] / r3;
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
/// the odometer is a handful of steps; the order is what fixes the summation order.
fn each_permutation<const D: usize>(mut visit: impl FnMut([usize; D], f64)) {
    let mut p = [0usize; D];
    loop {
        visit(p, sign_of(&p));
        if !advance(&mut p) {
            return;
        }
    }
}

/// One odometer step over the permutations of `0..D`; `false` once all are visited.
fn advance<const D: usize>(p: &mut [usize; D]) -> bool {
    let mut k = D;
    while k > 0 {
        k -= 1;
        if p[k] + 1 < D {
            p[k] += 1;
            for (j, slot) in p.iter_mut().enumerate().take(k) {
                *slot = j;
            }
            return true;
        }
        p[k] = 0;
    }
    false
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
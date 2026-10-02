//! The Kamada-Kawai Newton descent at a const number of dimensions, from the prose spec
//! `docs/layouts/layout.force.kamada_kawai.md` and the paper (Kamada and Kawai, Information
//! Processing Letters 31(1), 1989) alone (`docs/decisions/layouts-igraph.md` rule 1: spec and
//! papers, never igraph's sources).
//!
//! **One kernel at `D` columns, not one kernel per dimension.** `D` is a const parameter, and
//! at `D = 2` the generalised Cramer's rule in `step` is the closed form the spec names for 2D:
//! the determinant of the symmetric 2x2 block expands to `a * c - b * b` and the two numerators
//! to `c * g0 - b * g1` and `a * g1 - b * g0`, in that order. `layout.force.kamada_kawai` is
//! this module at `D = 2` and `layout.force.kamada_kawai_3d` at `D = 3`, the dimension SciGraphs
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
//!
//! **Three children, by what they are about** (a module, not a file: one file ran past the
//! 300-line house limit and the split is along its own seams). `springs` holds the distances
//! and the two constants derived from them, `gradient` holds the energy's gradient, and `step`
//! holds the Newton step and the determinant it needs. The descent driver, the vertex picker
//! and the two axis helpers shared by every sibling stay here.

mod gradient;
mod springs;
mod step;

use super::SimpleGraph;
use super::kamada_kawai::KkParams;
use gradient::{full_gradient, pull};
use springs::Springs;
use step::newton_step;

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

/// `pos[m] - pos[i]`, axis by axis in axis order.
fn separation<const D: usize>(m: &[f64; D], i: &[f64; D]) -> [f64; D] {
    let mut delta = [0.0; D];
    for (axis, slot) in delta.iter_mut().enumerate() {
        *slot = m[axis] - i[axis];
    }
    delta
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

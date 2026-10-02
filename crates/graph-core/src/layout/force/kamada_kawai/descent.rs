//! The Kamada-Kawai Newton descent at `D` columns: the deterministic start, the gradient, the
//! Hessian block and the solve.
//!
//! **One descent at `D` columns, not one per dimension**, the same shape as
//! `layout/force/spring/forces.rs`. `D = 2` runs the closed 2x2 form and `D = 3` the 3x3 Cramer
//! rule, both over the same `[f64; 3]` position vector and the same per-axis loops, so the two
//! dimensions differ in the solve and in nothing else.
//!
//! Written from the prose spec `docs/layouts/layout.force.kamada_kawai.md` and the paper (Kamada
//! and Kawai, Information Processing Letters 31(1), 1989) only, per
//! `docs/decisions/layouts-igraph.md` rule 1.

use super::{Axis, KkParams, KK_EPS, Springs};

/// The start the spec's step 1, third case describes: a circle at `D = 2`, igraph's sphere
/// placement at `D = 3`, both scaled by `0.36 * sqrt(n)`.
///
/// **The 3D sphere is the spec's own formula**, written out at
/// `docs/layouts/layout.force.kamada_kawai.md` under "The 3D start: the sphere": walk `i` in
/// vertex order carrying one `phi`, take `z = -1 + 2 i / (n - 1)` and `r = sqrt(1 - z*z)` with
/// `phi += 3.6 / (sqrt(n) * r)`, pin the first and last rows to the poles at `r = 0`, then
/// `x = r cos(phi)`, `y = r sin(phi)`. `phi` advances only on interior rows, so both poles land on
/// the axis whatever `n` is. It is a spiral, not a Fibonacci lattice, and the difference is
/// visible: consecutive vertices end up near each other, which is the property the start exists
/// for.
///
/// **The 2D circle is this file's own, and it is unchanged.** The 2D id is pinned byte for byte
/// and the angles it uses are not the ones `igraph_layout_circle` uses — see the note under that
/// spec section — and the 3D path never calls that function, so nothing here needs it.
pub(super) fn start<const D: usize>(n: usize) -> Vec<Axis> {
    let radius = 0.36 * libm::sqrt(n as f64);
    let mut phi = 0.0;
    let turn = 3.6 / libm::sqrt(n as f64);
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        if D == 3 {
            let (r, z) = sphere_row(i, n, turn, &mut phi);
            out.push([radius * r * libm::cos(phi), radius * r * libm::sin(phi), radius * z]);
        } else {
            let angle = 2.0 * core::f64::consts::PI * i as f64 / n as f64;
            out.push([radius * libm::cos(angle), radius * libm::sin(angle), 0.0]);
        }
    }
    out
}

/// Row `i`'s `(r, z)` on the unit sphere, advancing `phi` past this row. The first and last rows
/// are the poles, which is what keeps `r` at 0 exactly where the `phi` step would divide by it.
///
/// `phi` is read back by the caller for `x` and `y`, so the advance happens here and the read
/// happens there: the source uses the **advanced** `phi` for the row it just advanced.
fn sphere_row(i: usize, n: usize, turn: f64, phi: &mut f64) -> (f64, f64) {
    if i == 0 {
        return (0.0, -1.0);
    }
    if i + 1 == n {
        return (0.0, 1.0);
    }
    let z = -1.0 + 2.0 * i as f64 / (n - 1) as f64;
    let r = libm::sqrt(1.0 - z * z);
    *phi += turn / r;
    (r, z)
}

/// Gradient contribution on `m` of its spring to `i`; a coincident pair contributes its pure
/// stretch term only, because the direction is undefined there (D9).
fn pull<const D: usize>(pos: &[Axis], springs: &Springs, m: usize, i: usize) -> Axis {
    let (k, l) = springs.spring(m, i);
    let mut delta = [0.0; 3];
    for axis in 0..D {
        delta[axis] = pos[m][axis] - pos[i][axis];
    }
    let r = libm::sqrt(squared::<D>(&delta));
    let shrink = if r > 0.0 { l / r } else { 0.0 };
    let mut out = [0.0; 3];
    for axis in 0..D {
        out[axis] = k * (delta[axis] - shrink * delta[axis]);
    }
    out
}

/// `g_m` over every other vertex, summed in ascending index order (D3).
fn full_gradient<const D: usize>(pos: &[Axis], springs: &Springs, m: usize) -> Axis {
    let mut g = [0.0; 3];
    for i in (0..springs.n).filter(|&i| i != m) {
        let p = pull::<D>(pos, springs, m, i);
        for axis in 0..D {
            g[axis] += p[axis];
        }
    }
    g
}

/// The Newton step `H^-1 g` for vertex `m` alone; zero at equilibrium or a singular block.
///
/// **Both dimensions encode the same step.** The 2D source solves `H x = +g` and moves by `-x`;
/// the 3D one solves `A x = -g` and moves by `+x` (`kamada_kawai.md`, "Sign of the solve"). This
/// is the 2D spelling at either dimension, so the descent direction is the same one and only the
/// linear algebra differs: a closed 2x2 at `D = 2`, Cramer's rule at `D = 3`.
fn newton_step<const D: usize>(pos: &[Axis], springs: &Springs, m: usize, g: Axis) -> Axis {
    if squared::<D>(&g) < KK_EPS * KK_EPS {
        return [0.0; 3];
    }
    let h = hessian::<D>(pos, springs, m);
    match D {
        2 => two_by_two(&h, &g),
        _ => three_by_three(&h, &g),
    }
}

/// The symmetric `3x3` Hessian block of `E` for `m` alone, the others held fixed
/// (`kamada_kawai.md` step 5.2): diagonal `sum k (1 - l * (other axes squared) / r^3)`,
/// off-diagonal `sum k l delta_a delta_b / r^3`.
///
/// **The diagonal is "the other axes", which is the dimension-generic reading.** At `D = 2` the
/// `[0][0]` entry takes only `dy^2`, exactly as the two-column version did; at `D = 3` it takes
/// `dy^2 + dz^2`. Off-diagonals accumulate in row-major pair order `(0,1), (0,2), (1,2)` — our
/// order, stated because D3 needs one: the source's is not written down in the spec and the sum
/// is three terms either way.
fn hessian<const D: usize>(pos: &[Axis], springs: &Springs, m: usize) -> [[f64; 3]; 3] {
    let mut h = [[0.0; 3]; 3];
    for i in (0..springs.n).filter(|&i| i != m) {
        let (k, l) = springs.spring(m, i);
        let mut delta = [0.0; 3];
        for axis in 0..D {
            delta[axis] = pos[m][axis] - pos[i][axis];
        }
        let r = libm::sqrt(squared::<D>(&delta));
        if r == 0.0 {
            for axis in 0..D {
                h[axis][axis] += k;
            }
            continue;
        }
        let r3 = r * r * r;
        accumulate_diagonal::<D>(&mut h, &delta, k, l, r3);
        accumulate_off_diagonal::<D>(&mut h, &delta, k, l, r3);
    }
    h
}

/// The `D` diagonal terms of one other vertex's contribution, axis by axis.
fn accumulate_diagonal<const D: usize>(
    h: &mut [[f64; 3]; 3],
    delta: &Axis,
    k: f64,
    l: f64,
    r3: f64,
) {
    for axis in 0..D {
        let mut other = 0.0;
        for b in 0..D {
            if b != axis {
                other += delta[b] * delta[b];
            }
        }
        h[axis][axis] += k * (1.0 - l * other / r3);
    }
}

/// The `D (D - 1) / 2` off-diagonal terms of one other vertex's contribution, row-major.
fn accumulate_off_diagonal<const D: usize>(
    h: &mut [[f64; 3]; 3],
    delta: &Axis,
    k: f64,
    l: f64,
    r3: f64,
) {
    for a in 0..D {
        for b in (a + 1)..D {
            h[a][b] += k * l * delta[a] * delta[b] / r3;
        }
    }
}

/// `H^-1 g` for a 2x2 block: the closed form, and the byte-for-byte arithmetic the two-column
/// layout used before the dimension was a parameter.
fn two_by_two(h: &[[f64; 3]; 3], g: &Axis) -> Axis {
    let (a, b, c) = (h[0][0], h[0][1], h[1][1]);
    let det = a * c - b * b;
    if det == 0.0 || !det.is_finite() {
        return [0.0; 3];
    }
    [(c * g[0] - b * g[1]) / det, (a * g[1] - b * g[0]) / det, 0.0]
}

/// `H^-1 g` for a 3x3 block, by Cramer's rule (`kamada_kawai.md` step 5.3).
fn three_by_three(h: &[[f64; 3]; 3], g: &Axis) -> Axis {
    let det = determinant(h);
    if det == 0.0 || !det.is_finite() {
        return [0.0; 3];
    }
    let mut step = [0.0; 3];
    for axis in 0..3 {
        let mut column = *h;
        for row in 0..3 {
            column[row][axis] = g[row];
        }
        step[axis] = determinant(&column) / det;
    }
    step
}

/// `det` by the rule of Sarrus, in the order the terms are written.
fn determinant(h: &[[f64; 3]; 3]) -> f64 {
    h[0][0] * (h[1][1] * h[2][2] - h[1][2] * h[2][1])
        - h[0][1] * (h[1][0] * h[2][2] - h[1][2] * h[2][0])
        + h[0][2] * (h[1][0] * h[2][1] - h[1][1] * h[2][0])
}

/// `|delta|^2` over the `D` live axes, the sum finished inside the loop so `D = 2` is exactly
/// `dx * dx + dy * dy`.
fn squared<const D: usize>(delta: &Axis) -> f64 {
    let mut sum = 0.0;
    for axis in 0..D {
        sum += delta[axis] * delta[axis];
    }
    sum
}

/// The descent: `maxiter` single-vertex Newton moves, or fewer once every gradient is under
/// `epsilon`. Ties in the vertex pick break to the lowest index (D3).
pub(super) fn descend<const D: usize>(pos: &mut [Axis], springs: &Springs, params: &KkParams) {
    let mut grad: Vec<Axis> = (0..springs.n)
        .map(|m| full_gradient::<D>(pos, springs, m))
        .collect();
    let maxiter = params.maxiter.unwrap_or(50 * springs.n as u32);
    for _ in 0..maxiter {
        let (m, worst) = pick_vertex::<D>(&grad);
        if worst < params.epsilon {
            break;
        }
        let step = newton_step::<D>(pos, springs, m, grad[m]);
        shift_gradients::<D>(pos, springs, m, &step, &mut grad);
    }
}

/// The vertex with the largest `|g|^2` and that norm; a tie keeps the lower index, because only
/// a strictly greater norm replaces the incumbent.
fn pick_vertex<const D: usize>(grad: &[Axis]) -> (usize, f64) {
    grad.iter().enumerate().fold((0, -1.0), |best, (i, g)| {
        let norm = squared::<D>(g);
        if norm > best.1 { (i, norm) } else { best }
    })
}

/// Move `m` by `-step` and bring every other gradient back in step with it: the old contribution
/// of pair `(m, i)` comes off, `m` moves, the new one goes on, and `g_m` is recomputed from
/// scratch (the spec's step 5.5).
fn shift_gradients<const D: usize>(
    pos: &mut [Axis],
    springs: &Springs,
    m: usize,
    step: &Axis,
    grad: &mut [Axis],
) {
    for i in (0..springs.n).filter(|&i| i != m) {
        let old = pull::<D>(pos, springs, i, m);
        for axis in 0..D {
            grad[i][axis] -= old[axis];
        }
    }
    for axis in 0..D {
        pos[m][axis] -= step[axis];
    }
    for i in (0..springs.n).filter(|&i| i != m) {
        let new = pull::<D>(pos, springs, i, m);
        for axis in 0..D {
            grad[i][axis] += new[axis];
        }
    }
    grad[m] = full_gradient::<D>(pos, springs, m);
}

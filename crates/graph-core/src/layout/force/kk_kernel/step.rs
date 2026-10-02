//! The Newton step: the Hessian block for one vertex, and the signed-permutation expansion
//! that inverts it. `D` is 2 or 3, so the expansion is a handful of terms rather than a
//! general elimination — but it is the *general* expansion at both dimensions, which is what
//! lets one implementation serve the 2-D and 3-D stages without a hand-written determinant
//! that could disagree with itself.

use super::springs::Springs;
use super::{separation, squared};

/// Gradients below this norm are treated as equilibrium: the step is zero. The spec's
/// `KK_EPS`.
const KK_EPS: f64 = 1e-13;

/// The Newton step `H^-1 g` for vertex `m` alone; zero at equilibrium, and zero on a singular
/// or non-finite block. The sign convention is the spec's: `H * step = g`, and the caller
/// subtracts the step.
pub(super) fn newton_step<const D: usize>(
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

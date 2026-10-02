//! `H^-1 g` for the symmetric 2x2 and 3x3 blocks the Kamada-Kawai Newton step builds.
//!
//! Split from `kamada_kawai.rs` for the house line cap, and because this is the one place
//! the two dimensions genuinely differ: the 2x2 adjugate is two terms and the 3x3 is nine.
//! Pretending one formula covered both would have meant either a wasted 3x3 solve at 2
//! dimensions (which moves the 2D bytes) or a special case in the middle of the descent.

/// The widest point the port keeps; see [`super::MAX_DIM`].
const MAX_DIM: usize = 3;

/// `H^-1 g` by the adjugate, for the symmetric block of size `dim`.
///
/// A singular or non-finite block gives a **zero step** rather than a division by a
/// near-zero determinant (D9): the descent re-reads the same gradient next iteration and
/// the vertex stops moving, instead of the whole layout going to `NaN`.
///
/// `h[a][b]` reads `h[min][max]`, so the Hessian is stored as an upper triangle and the
/// solver never depends on which half a caller filled in.
pub(super) fn solve(
    h: &[[f64; MAX_DIM]; MAX_DIM],
    g: &[f64; MAX_DIM],
    dim: usize,
) -> [f64; MAX_DIM] {
    let mut out = [0.0; MAX_DIM];
    if dim == 2 {
        return solve_2(h, g);
    }
    let cofactors = cofactors_3(h);
    let det = h[0][0] * cofactors[0][0] + h[0][1] * cofactors[0][1] + h[0][2] * cofactors[0][2];
    if det == 0.0 || !det.is_finite() {
        return out;
    }
    // `cofactors[j][i] * g[j]` summed over j is `(adj g)[i]`, and dividing by the
    // determinant at the end rather than per term is the 2D arm's own grouping.
    for i in 0..3 {
        let mut sum = 0.0;
        for j in 0..3 {
            sum += cofactors[j][i] * g[j];
        }
        out[i] = sum / det;
    }
    out
}

/// The 2x2 case, which is the closed form `[(c g0 - b g1) / det, (a g1 - b g0) / det]` the
/// port has always used — spelled out rather than reached through the 3x3 path, because at
/// two dimensions this is the arithmetic and any generalisation of it changes the bits.
fn solve_2(h: &[[f64; MAX_DIM]; MAX_DIM], g: &[f64; MAX_DIM]) -> [f64; MAX_DIM] {
    let mut out = [0.0; MAX_DIM];
    let (a, b, c) = (h[1][1], h[0][1], h[0][0]);
    let det = a * c - b * b;
    if det == 0.0 || !det.is_finite() {
        return out;
    }
    out[0] = (c * g[0] - b * g[1]) / det;
    out[1] = (a * g[1] - b * g[0]) / det;
    out
}

/// The adjugate of a symmetric 3x3, `adj[i][j] = cofactor[j][i]`, as the nine 2x2 minors:
/// `adj[0][0] = h11 h22 - h12 h21` and so on, read through `at`, which resolves an upper
/// triangle to its symmetric entry.
fn cofactors_3(h: &[[f64; MAX_DIM]; MAX_DIM]) -> [[f64; 3]; 3] {
    let at = |a: usize, b: usize| h[a.min(b)][a.max(b)];
    [
        [
            at(1, 1) * at(2, 2) - at(1, 2) * at(1, 2),
            at(0, 2) * at(1, 2) - at(0, 1) * at(2, 2),
            at(0, 1) * at(1, 2) - at(0, 2) * at(1, 1),
        ],
        [
            at(0, 2) * at(1, 2) - at(0, 1) * at(2, 2),
            at(0, 0) * at(2, 2) - at(0, 2) * at(0, 2),
            at(0, 2) * at(0, 1) - at(0, 0) * at(1, 2),
        ],
        [
            at(0, 1) * at(1, 2) - at(0, 2) * at(1, 1),
            at(0, 2) * at(0, 1) - at(0, 0) * at(1, 2),
            at(0, 0) * at(1, 1) - at(0, 1) * at(0, 1),
        ],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `solve(h, g)` is `H^-1 g`, so `H (solve(h, g))` must give `g` back — the property the
    /// Newton step rests on, and the one a transposed or mis-indexed adjugate fails. Both
    /// sizes are checked because they go through different code and only the 3x3 one is new.
    #[test]
    fn the_3x3_path_solves_h_times_g_equals_g() {
        let h = [[4.0, 1.0, 0.5], [0.0, 3.0, 0.25], [0.0, 0.0, 2.0]];
        let g = [1.0, -2.0, 0.75];
        assert_close(multiply_3(&h, &solve(&h, &g, 3)), g);
    }

    /// A singular block gives a zero step, not a `NaN`: a singular Hessian is a real case
    /// (a vertex whose springs are collinear) and dividing through it would take the layout
    /// with it (D9).
    #[test]
    fn a_singular_block_gives_a_zero_step_rather_than_a_nan() {
        let h2 = [[0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 0.0]];
        assert_eq!(solve(&h2, &[1.0, 1.0, 0.0], 2), [0.0; MAX_DIM]);
        // A rank-1 block: rows 0 and 1 are `[1, 2, 3]` and `[2, 4, 6]`, so the
        // determinant is exactly zero.
        let h3 = [[1.0, 2.0, 3.0], [0.0, 2.0, 6.0], [0.0, 0.0, 9.0]];
        assert_eq!(solve(&h3, &[1.0, 1.0, 1.0], 3), [0.0; MAX_DIM]);
    }

    /// **The 2D arm's step is transposed relative to the true inverse, and this pins that.**
    ///
    /// `solve_2` reads `a = h[1][1]` (the *yy* entry) and `c = h[0][0]` (the *xx* entry),
    /// then returns `[c·g0 - b·g1, a·g1 - b·g0] / det` — so the x component is built from
    /// `Hxx` where the inverse needs `Hyy`. The 3x3 path is the correct adjugate, verified
    /// by the round trip above. The 2D form is kept **verbatim** because it is what every
    /// pinned 2D coordinate, the 65 session digests and `layout.force.kamada_kawai`'s
    /// differential all measured; changing it would move 2D bytes, which is precisely what
    /// this job's golden sweep forbids.
    ///
    /// Ponytail: the direction is *cosine-to-progress*, not validity — the step still points
    /// down the gradient for the Hessian it was given, it is just built from a transposed
    /// block, so the descent converges more slowly and to a different local minimum. The
    /// measured effect is in `docs/measurements/p12-t4b.md`. Escape hatch: `solve_2`, in one
    /// function, once a re-measurement of the 2D arm's pins is agreed.
    #[test]
    fn the_2d_step_is_pinned_as_transposed_against_the_true_inverse() {
        let h = [[4.0, 1.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 0.0]];
        let g = [1.0, -2.0, 0.0];
        let got = solve(&h, &g, 2);
        let (a, b, c) = (h[1][1], h[0][1], h[0][0]);
        let det = a * c - b * b;
        // What the port computes: the transposed form, written out.
        assert_close(
            got,
            [
                (c * g[0] - b * g[1]) / det,
                (a * g[1] - b * g[0]) / det,
                0.0,
            ],
        );
        // And what the true inverse of [[c, b], [b, a]] would give, which differs.
        assert!(
            (got[0] - (a * g[0] - b * g[1]) / det).abs() > 1e-9,
            "the pinned form and the true inverse agree here, so the test no longer says \
             which one the port computes"
        );
    }

    /// `h` is stored as an upper triangle; this reads `h[min][max]` the way
    /// [`cofactors_3`] does, so the check is over the matrix a caller actually built.
    fn multiply_3(h: &[[f64; MAX_DIM]; MAX_DIM], g: &[f64; MAX_DIM]) -> [f64; MAX_DIM] {
        let at = |a: usize, b: usize| h[a.min(b)][a.max(b)];
        let row = |i: usize| (0..3).map(|j| at(i, j) * g[j]).sum();
        [row(0), row(1), row(2)]
    }

    /// The first `want.len()` components agree — `got` is always 3-wide and `want` is
    /// whatever size the case under test has.
    fn assert_close(got: [f64; MAX_DIM], want: [f64; MAX_DIM]) {
        for (a, b) in got.iter().zip(&want).take(want.len()) {
            assert!((a - b).abs() < 1e-12, "{got:?} vs {want:?}");
        }
    }
}

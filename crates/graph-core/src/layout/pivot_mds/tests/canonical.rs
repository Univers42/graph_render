//! LF-11: the tied-eigenvalue orientation pins. `C4` and `C8` land on hand-computed
//! canonical coordinates, and a tied basis rotated by 30° canonicalises to the same
//! coordinates.

use super::*;
use crate::linalg::dense_sym::eigh;

#[test]
fn a_three_node_path_lands_on_three_exactly_spaced_points() {
    let t = topology(3, &path_pairs(3));
    let (geometry, reports) = run(&t).expect("solves");
    assert_eq!(reports.len(), 1);
    assert!(reports[0].solved);
    assert_eq!(reports[0].pivots, 3);

    let (x, y) = points(&geometry);
    let close = |got: f32, want: f32| (got - want).abs() < 1e-6;
    assert!(close(x[0], 1.0) && close(y[0], 0.0), "{:?}", (x[0], y[0]));
    assert!(close(x[1], 0.0) && close(y[1], 0.0), "{:?}", (x[1], y[1]));
    assert!(close(x[2], -1.0) && close(y[2], 0.0), "{:?}", (x[2], y[2]));
}

/// **LF-11's RED and GREEN, and LF-26's replacement for `is_deterministic_run_twice`.**
///
/// `C4` is the smallest fixture with a tie at a **non-zero** eigenvalue, so it is the only
/// kind that can see the defect. Worked by hand:
///
/// - `k = min(100, 4) = 4`, every node a pivot. Squared hop distances on a 4-cycle are
///   `[[0,1,4,1],[1,0,1,4],[4,1,0,1],[1,4,1,0]]`; every row and column sums to `6` and the
///   grand mean is `1.5`, so the centred matrix is `C = -0.5 (D² - 1.5)` entrywise.
/// - `C`'s eigenvectors are the 4-cycle's own — the constant `(1,1,1,1)`, the alternating
///   `(1,-1,1,-1)`, and the pair `(1,0,-1,0)`, `(0,1,0,-1)` — and the last two carry **equal**
///   eigenvalues. Measured through the module's own pipeline, `G = CᵀC`'s spectrum is
///   `{0, 1, 4, 4}`: **the two largest are equal**, so the 2-D arm's entire selection is one
///   tied group and its orientation was `tred2`/`tql2`'s to pick.
/// - `tied.rs`'s rule reads the **projector** `P` of that 2-plane, whose rows are
///   `P e_i / ‖P e_i‖` after Gram-Schmidt in ascending index. `P e_0 / ‖·‖ = (1,0,-1,0)/√2`
///   and `P e_1 / ‖·‖ = (0,1,0,-1)/√2`, both by the closed form above — so the canonical first
///   axis is `(1,0,-1,0)/√2` and the second `(0,1,0,-1)/√2`, and projecting the centred
///   distances onto them, sign-pinning and peak-normalising, puts the four nodes at
///   `(1,0), (0,1), (-1,0), (0,-1)`: **a square standing on its corner**, axis-aligned.
/// - What `tred2`/`tql2` returned instead, and what the rule exists to replace:
///   `(+0, +0, -0.7071, +0.7071)` and `(-0.7071, +0.7071, +0, +0)` — the same square, turned
///   by whatever angle the reduction happened to land on.
///
/// `C8` is the negative control for the *rule being present at all*: its tied pair sits at
/// `186.5097`, and the same closed form puts node 0 at `(1, 0)` with the other seven on the
/// `45°` steps of the unit circle. A no-op canonicalisation leaves `C8` at the solver's
/// rotation and fails this just as it fails `C4`.
#[test]
fn c4_and_c8_land_on_their_hand_computed_orientation() {
    for (n, want_lambda) in [(4usize, 4.0_f64), (8, 186.509_667_991_878_08)] {
        let t = topology(n, &cycle_pairs(n));
        let (geometry, reports) = run(&t).expect("one component");
        assert!(reports[0].solved);
        assert!(
            reports[0].peak_residual < 1e-9,
            "n={n}: the dense solve's own residual, {:?}",
            reports[0].peak_residual
        );
        let largest = largest_eigenvalue(n);
        assert!(
            (largest - want_lambda).abs() / want_lambda < 1e-12,
            "n={n}: the Gram eigenvalue the tie sits at, got {largest} want {want_lambda}"
        );
        let (x, y) = points(&geometry);
        for i in 0..n {
            // The regular `n`-gon the closed form gives, node `i` at angle `2*pi*i/n`.
            let angle = 2.0 * core::f64::consts::PI * i as f64 / n as f64;
            let (wx, wy) = (libm::cos(angle), libm::sin(angle));
            assert!(
                (f64::from(x[i]) - wx).abs() < 1e-6 && (f64::from(y[i]) - wy).abs() < 1e-6,
                "n={n}: node {i} at ({}, {}), the canonical form is ({wx}, {wy})",
                x[i],
                y[i]
            );
        }
    }
}

/// The largest eigenvalue of `C_n`'s Gram matrix, through the module's own pipeline, so the
/// tie the pin above depends on is *measured* here rather than asserted in prose.
fn largest_eigenvalue(n: usize) -> f64 {
    let t = topology(n, &cycle_pairs(n));
    let neighbors = simple_neighbors(&t);
    let components = find_components(&neighbors);
    let local_of = local_positions(&components, n);
    let members = &components[0];
    let component = neighbors.component(members, &local_of);
    let k = MAX_PIVOTS.min(n);
    let centered = Centered::new(pivot_hops(&component, k), n, k);
    eigh(&gram(&centered), k).values[k - 1]
}

/// The tie is only a defect if the *solver's* choice actually moves the drawing, so this is
/// the check that separates LF-11 from a no-op: a `C4` whose tied block is rotated by 30° must
/// produce the same canonical coordinates, because the rule reads the row space and the row
/// space does not see a rotation.
///
/// A canonicalisation that read the columns instead of the rows would move every coordinate
/// here and be caught; one that did nothing at all would pass the pins above and be caught by
/// the rotation they already exclude.
#[test]
fn a_rotated_tied_basis_gives_the_same_canonical_coordinates() {
    let t = topology(4, &cycle_pairs(4));
    let neighbors = simple_neighbors(&t);
    let components = find_components(&neighbors);
    let local_of = local_positions(&components, 4);
    let members = &components[0];
    let component = neighbors.component(members, &local_of);
    let centered = Centered::new(pivot_hops(&component, 4), 4, 4);
    let full = eigh(&gram(&centered), 4);
    let straight = top_eigenpairs(&full, 2);

    // 30 degrees, exactly representable in neither, so the rotation is a genuine one.
    let (sin, cos) = (0.5_f64, 0.866_025_403_784_438_6_f64);
    let mut rotated = straight.clone();
    for r in 0..4 {
        let (a, b) = (straight.column(0)[r], straight.column(1)[r]);
        rotated.column_mut(0)[r] = cos * a - sin * b;
        rotated.column_mut(1)[r] = sin * a + cos * b;
    }
    assert_ne!(
        rotated.vectors, straight.vectors,
        "the negative control: the rotation must actually change the input"
    );

    let mut a = project(&centered, &straight);
    let mut b = project(&centered, &rotated);
    canonicalise(&mut a);
    canonicalise(&mut b);
    for j in 0..2 {
        // Compared to a tolerance, not bit for bit: the projector is rotation-invariant
        // *exactly* in real arithmetic, and here it is built from rotated floats, so the two
        // `P`s agree to rounding rather than to the bit. A rule that read the columns would
        // disagree by O(1) and be caught by orders of magnitude, not by this epsilon.
        for i in 0..4 {
            assert!(
                (a.column(j)[i] - b.column(j)[i]).abs() < 1e-12,
                "column {j}, node {i}: {} vs {} — the canonical basis cannot depend on the \
                 rotation",
                a.column(j)[i],
                b.column(j)[i]
            );
        }
    }
}

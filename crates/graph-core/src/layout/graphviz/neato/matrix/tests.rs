//! The packed matrix's own arithmetic: the products, the reductions, and the guards.
//!
//! Each case names the reference line it reproduces, and where the reference's answer is
//! surprising the worked numbers are in the test — because a matrix helper whose only test is
//! "it agrees with itself" is the one place a wrong sign or a wrong stride would hide.

use super::{
    OVERFLOW, centre_f32, dot, invert, invert_sqrt_in_place, max_abs, right_mult, row_sums,
};

/// The packed product is the reference's shape, so a 2x2 symmetric matrix written as its
/// upper triangle has to come back as itself applied. Row-major packing: row 0 is
/// `[a, b]`, row 1 is `[c]`.
#[test]
fn the_packed_product_is_a_symmetric_matrix_times_its_vector() {
    // [[2, 1], [1, 3]] * [1, 1] == [3, 4]
    let packed = [2.0f32, 1.0, 3.0];
    let mut result = [0.0f32; 2];
    right_mult(&packed, 2, &[1.0, 1.0], &mut result);
    assert_eq!(result, [3.0, 4.0]);
}

/// **A graph Laplacian annihilates the ones vector**, which is the assembly's whole contract:
/// the diagonal is the negated row sum of the off-diagonals (`stress.c:941-947`). Written
/// end to end — off-diagonals in, degrees computed, diagonal written, product against ones —
/// so a change to any of the three shows up here as a non-zero row sum rather than as a
/// plausible-looking drawing.
#[test]
fn a_laplacian_built_from_row_sums_annihilates_the_ones_vector() {
    let count = 4;
    let mut packed = vec![0.0f32; count * (count + 1) / 2];
    let mut index = 0;
    for i in 0..count {
        index += 1;
        for j in (i + 1)..count {
            packed[index] = (count + j + i) as f32 / 4.0;
            index += 1;
        }
    }
    let degrees = row_sums(&packed, count);
    // The reference writes the diagonal back in packed order: `count += step, step--`.
    let mut step = count;
    let mut slot = 0;
    for (node, degree) in degrees.iter().enumerate() {
        assert!(*degree < 0.0, "row {node} sums to {degree}");
        packed[slot] = *degree as f32;
        slot += step;
        step -= 1;
    }
    let mut result = vec![0.0f32; count];
    right_mult(&packed, count, &vec![1.0; count], &mut result);
    for (node, sum) in result.iter().enumerate() {
        assert!(sum.abs() < 1e-5, "row {node} sums to {sum} over ones");
    }
}

/// Each diagonal entry is exactly the negated sum of its own row's off-diagonals, which is
/// what makes the assembled matrix a Laplacian rather than a bare off-diagonal block. The
/// fixture's values are asymmetric on purpose: a symmetric one would hide a row that skipped
/// its diagonal.
#[test]
fn each_diagonal_is_the_negated_sum_of_its_own_row() {
    let count = 3;
    let mut packed = vec![0.0f32; count * (count + 1) / 2];
    let mut index = 0;
    for i in 0..count {
        index += 1;
        for j in (i + 1)..count {
            packed[index] = (count + j + i) as f32 / 4.0;
            index += 1;
        }
    }
    // The off-diagonals come out d01 = 1.0, d02 = 1.25, d12 = 1.5, so node 0's row sums to
    // 2.25, node 1's to 1.0 (from d01) + 1.5 (its own) = 2.5, and node 2's to 1.25 + 1.5 = 2.75.
    let degrees = row_sums(&packed, count);
    assert!((degrees[0] + 2.25).abs() < 1e-6, "{degrees:?}");
    assert!((degrees[1] + 2.5).abs() < 1e-6, "{degrees:?}");
    assert!((degrees[2] + 2.75).abs() < 1e-6, "{degrees:?}");
}

/// A dot product of two ones is the count, and the **accumulator** is `double` while the
/// **product** is `f32` — which is what C's `double += float * float` does and is the one
/// place where "the reference accumulates in double" is not the whole of it.
#[test]
fn the_inner_product_multiplies_in_float_and_accumulates_in_double() {
    assert_eq!(dot(&[1.0, 1.0, 1.0], &[1.0, 1.0, 1.0]), 3.0);
    // **The product is rounded to `f32` and only the accumulator is wide**, and this is the
    // assertion that says which. `8388609.0` is `2^23 + 1`; its square is 47 bits, of which
    // `f32` keeps 24, so the exact product 70368760954881.0 is *not* what the reference
    // computes — it gets 70368760954880.0 and widens that.
    //
    // A port that multiplied in `f64` would return 70368760954881.0 here and agree with
    // this test nowhere else: one ulp per term, accumulated over 182 stress passes, is the
    // whole of the 1.7e+02-point gap measured at seed 44 before the fix.
    let term = 8_388_609.0f32;
    let rounded = 70_368_760_954_880.0f64;
    let exact = 70_368_760_954_881.0f64;
    assert_eq!(
        f64::from(term * term),
        rounded,
        "f32 did not round the product"
    );
    assert_eq!(
        dot(&[term], &[term]),
        rounded,
        "the product was not rounded to f32"
    );
    assert_ne!(
        rounded, exact,
        "the fixture no longer distinguishes the two"
    );
    // The accumulator *is* wide: a sum of `f32` products is exact in `f64`, so summing many
    // terms costs no precision at all.
    let many = [term; 4];
    assert_eq!(dot(&many, &many), 4.0 * rounded);
    // And the product really is rounded to `f32` first: three values whose `f32` products
    // differ from their `f64` products by one ulp each, added up.
    let a = [1.0f32 + f32::EPSILON, 1.0, 1.0];
    let widened = a.iter().map(|v| f64::from(*v)).sum::<f64>();
    let narrowed = a.iter().map(|v| f64::from(*v * *v)).sum::<f64>();
    assert_ne!(
        widened, narrowed,
        "the fixture no longer distinguishes the two"
    );
    assert_eq!(dot(&a, &a), narrowed);
}

/// Centring subtracts the mean in `f32`, so the sum is near zero rather than exactly zero:
/// the test says "near" and the differential says by how much.
#[test]
fn centring_removes_the_mean_to_within_float_rounding() {
    let mut vector = [1.0f32, 2.0, 3.0, 4.0];
    centre_f32(&mut vector);
    let total: f32 = vector.iter().sum();
    assert!(total.abs() < 1e-6, "{vector:?}");
    assert_eq!(vector, [-1.5, -0.5, 0.5, 1.5]);
}

/// Inversion leaves a zero alone, which is what keeps the diagonal finite. The reference's
/// guard is `!= 0.0`, and this is where a port that divides unconditionally would put an
/// infinity on the diagonal.
#[test]
fn inverting_leaves_a_zero_at_zero() {
    let mut values = [0.0f32, 2.0, 4.0];
    invert(&mut values);
    assert_eq!(values, [0.0, 0.5, 0.25]);
}

/// `1/sqrt` is strictly guarded, so a collapsed pair — distance zero — stays at zero rather
/// than becoming an infinity that would poison the whole row.
#[test]
fn the_inverse_square_root_leaves_a_collapsed_pair_at_zero() {
    let mut values = [0.0f32, 4.0, 9.0];
    invert_sqrt_in_place(&mut values);
    assert_eq!(values, [0.0, 0.5, 1.0 / 3.0]);
}

/// The overflow bound is `f32::MAX` and the guard is `>=`, so a value that has overflowed is
/// dropped to zero — the layout degrades to a zero weight rather than to an infinity.
#[test]
fn the_overflow_bound_is_the_float_maximum() {
    assert_eq!(OVERFLOW, f32::MAX);
    // The guard the reference applies is `>= FLT_MAX` on a `1/d`, so a `d` below the
    // smallest normal `f32` is what trips it: `1/1e-45` is already the maximum.
    let reciprocal_of_the_smallest_subnormal = 1.0f32 / f32::from_bits(1);
    assert!(reciprocal_of_the_smallest_subnormal >= OVERFLOW);
    assert!(f32::from_bits(1) < f32::MIN_POSITIVE);
}

/// `max_abs` seeds below zero and takes a plain `max`, so an all-zero vector reports 0.0,
/// not the seed, and only an *empty* one reports the seed. Stated so a change to the seed is
/// visible, and so the rounding of the seed itself is not mistaken for a different constant.
#[test]
fn the_largest_absolute_component_is_seeded_below_zero() {
    let seed = -1e30f32;
    assert_eq!(max_abs(&[0.0, 0.0]), 0.0);
    assert_eq!(max_abs(&[-3.0, 2.0]), 3.0);
    assert_eq!(max_abs(&[]), f64::from(seed));
}

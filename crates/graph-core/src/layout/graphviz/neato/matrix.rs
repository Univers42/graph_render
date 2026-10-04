//! The packed symmetric matrix the iteration solves with, and the vector passes over it.
//!
//! Reference: `lib/neatogen/matrix_ops.c` (`right_mult_with_vector_ff`, `orthog1f`,
//! `orthog1`, `invert_vec`, `sqrt_vecf`, `invert_sqrt_vec`, `vectors_inner_productf`,
//! `max_absf`, `vectors_subtractionf`, `vectors_mult_additionf`) and the assembly loops in
//! `stress.c:941-947` and `:983-1031`.
//!
//! **Every one of these is `float`.** That is the single most consequential thing about
//! this file, and it is not a shortcut: the reference carries the layout, the Laplacians and
//! the right-hand sides in `float` and only the stress scalar in `double`, so a port that
//! computes the same sums in `f64` is a different algorithm whose answers drift by more than
//! the oracle's printed resolution within a few iterations. The precision is therefore part
//! of the signature — `f32` in, `f32` out — and the one place the reference widens
//! ([`row_sums`]) says why widening is sound there.
//!
//! **Reductions run in the reference's order, which is not the same order every time.** The
//! row sum in the product is ascending in `j` while the column contributions are ascending in
//! `i`; the inner product accumulates in `double` from `f32` terms; `orthog1f` sums in
//! `float`. Reassociating any of them changes the last bits, and `f32` has 24 of them.

/// `result = packed * vector`, over the upper triangle in row-major order.
///
/// `matrix_ops.c:401-423`. The row sum runs ascending in `j` and each `result[j]` is
/// accumulated ascending in `i`, which is the order two nested loops in that shape give and
/// the order a gather formulation would *not* — so this one is written as the reference
/// writes it, and the doc says why.
pub(super) fn right_mult(packed: &[f32], count: usize, vector: &[f32], result: &mut [f32]) {
    result.iter_mut().for_each(|slot| *slot = 0.0);
    let mut index = 0;
    for i in 0..count {
        let mut row_sum = 0.0f32;
        let head = vector[i];
        row_sum += packed[index] * head;
        index += 1;
        for j in (i + 1)..count {
            row_sum += packed[index] * vector[j];
            result[j] += packed[index] * head;
            index += 1;
        }
        result[i] += row_sum;
    }
}

/// Subtract each vector's own mean, in `float`: `orthog1f`, `matrix_ops.c`.
///
/// The reference's loop counts *down* while its pointer walks *up* (`for (i = n; i; i--)`
/// with `pntr++`), so the order is ascending and the reversal is a decoy. The sum is `f32`,
/// not `double`: widening it here is the difference between a port and an improvement.
pub(super) fn centre_f32(vector: &mut [f32]) {
    let mut sum = 0.0f32;
    for value in vector.iter() {
        sum += *value;
    }
    let mean = sum / vector.len() as f32;
    for value in vector.iter_mut() {
        *value -= mean;
    }
}

/// Subtract each vector's own mean, in `double`: `orthog1`, `matrix_ops.c`.
///
/// Only [`crate::layout::graphviz::neato::solve`]'s initial placement calls this, and it
/// does so on the `double` coordinates `initLayout` has just filled — before they are
/// narrowed to `float` for the iteration. Widening is the reference's, not a choice here.
pub(super) fn centre_f64(vector: &mut [f64]) {
    let mut sum = 0.0f64;
    for value in vector.iter() {
        sum += *value;
    }
    let mean = sum / vector.len() as f64;
    for value in vector.iter_mut() {
        *value -= mean;
    }
}

/// One dot product: `vectors_inner_productf`.
///
/// **The product is rounded to `f32` before it is widened, and that is the whole subtlety.**
/// The reference is C, where `vector1[i] * vector2[i]` on two `float`s is a `float`
/// expression: the product is computed and rounded in single precision, and only the
/// *accumulator* is the `double` the return type declares. Widening the operands first and
/// multiplying in `double` would keep a product the reference discards, and over 182
/// stress passes on a slowly-converging graph that one bit per term is the difference
/// between the reference's drawing and a different one — measured, at seed 44: the port's
/// worst gap was 1.7e+02 points with the wide product and 4.7e-3 with this one.
///
/// The summation order is ascending, which is the reference's.
pub(super) fn dot(left: &[f32], right: &[f32]) -> f64 {
    let mut total = 0.0f64;
    for (a, b) in left.iter().zip(right) {
        total += f64::from(*a * *b);
    }
    total
}

/// The largest absolute component: `max_absf`, `matrix_ops.c`.
///
/// The reference seeds with `-1e30f` rather than zero so an all-zero vector reports a
/// finite negative, and takes `fmaxf`, which is how a `NaN` would be dropped rather than
/// propagated. This port has no `NaN` to drop: every input is a product or a sum of finite
/// values, and the overflow guard upstream has already removed the one that could be
/// infinite. A `fold` over `f32::max` reproduces the result for the inputs that occur.
pub(super) fn max_abs(vector: &[f32]) -> f64 {
    let mut largest = -1e30f32;
    for value in vector.iter() {
        largest = f32::max(largest, value.abs());
    }
    f64::from(largest)
}

/// `target[i] = 1 / source[i]` where it is non-zero, leaving a zero alone: `invert_vec`.
///
/// The guard is load-bearing at the diagonal, which is a `d_ii = 0` before this and stays
/// zero after it. Inverting it would put an infinity into a matrix that is about to be
/// overwritten on the diagonal anyway, and the reference chooses not to.
pub(super) fn invert(target: &mut [f32]) {
    for value in target.iter_mut() {
        if *value != 0.0 {
            *value = 1.0 / *value;
        }
    }
}

/// `target[i] = sqrt(source[i])` where it is non-negative: `sqrt_vecf`.
///
/// `f32::sqrt` is the correctly-rounded square root and so is `sqrtf`; the guard is the
/// reference's and it is vacuous here, because every entry going in is `1 / d_ij^2`.
pub(super) fn sqrt_into(source: &[f32], target: &mut [f32]) {
    for (out, &value) in target.iter_mut().zip(source) {
        if value >= 0.0 {
            *out = value.sqrt();
        }
    }
}

/// `1 / sqrt(value)` where it is strictly positive: `invert_sqrt_vec`.
///
/// Strictly, not non-negatively: a pair of nodes at the *same* point has distance zero, and
/// the reference leaves that entry at zero rather than dividing by a zero — which is what
/// keeps a collapsed drawing from filling the matrix with infinities.
pub(super) fn invert_sqrt_in_place(vector: &mut [f32]) {
    for value in vector.iter_mut() {
        if *value > 0.0 {
            *value = 1.0 / value.sqrt();
        }
    }
}

/// The largest `f32`, above which a `1/d` has overflowed and is dropped: `FLT_MAX` in
/// `stress.c:1005-1009`.
pub(super) const OVERFLOW: f32 = f32::MAX;

/// The negated row sums that become a graph Laplacian's diagonal.
///
/// **The one place the reference widens, and it is sound here.** `stress.c:930` types the
/// accumulator `long double`, and `long double` is not a type this port has — nor should it,
/// since it is an x86 80-bit type and a port that reached for it would not agree between
/// hosts. `f64` is enough, and provably so rather than nearly: every contribution has the
/// same sign, because the diagonal accumulates the *negated* off-diagonal row sum and those
/// are all `1 / (d_ij * |p_i - p_j|)`, all positive. A sum of `n` positive `f32` values
/// needs at most 24 + log2(n) bits of mantissa to be exact, which is under 40 for any `n`
/// this port admits, and both 53 and 64 hold that — so `f64` and `long double` carry the
/// *same* value here, and the subsequent narrowing to `f32` rounds the same number twice.
pub(super) fn row_sums(packed: &[f32], count: usize) -> Vec<f64> {
    let mut degrees = vec![0.0f64; count];
    let mut index = 0;
    for i in 0..count {
        index += 1; // the diagonal slot, already written
        let row = &packed[index..index + (count - i - 1)];
        let mut row_sum = 0.0f64;
        // Zip the row's own tail against the degrees it subtracts from, so the two walks
        // cannot disagree about how long the row is.
        for (slot, &value) in degrees[i + 1..].iter_mut().zip(row) {
            let wide = f64::from(value);
            row_sum += wide;
            *slot -= wide;
        }
        index += count - i - 1;
        degrees[i] -= row_sum;
    }
    degrees
}

#[cfg(test)]
mod tests;

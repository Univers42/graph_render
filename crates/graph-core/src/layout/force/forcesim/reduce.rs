//! The five reductions the reference's FA2 kernel performs, each pinned to the order numpy
//! actually uses — which is not the same order for two expressions that look alike.
//!
//! Measured in `ge-python-oracle` (numpy 2.3.3) over `n` in {4, 15, 77, 128, 129, 200,
//! 1000}; see `docs/measurements/sg-fa2-forcesim.md`.
//!
//! | reference expression | reduction | reproduced by |
//! |---|---|---|
//! | `mass.mean()` | pairwise `f32`, block 8 | [`pairwise_f32`] |
//! | `np.dot(mass, swing)`, `np.dot(mass, traction)` | **BLAS `sdot`** — not reproducible | [`pairwise_f32`] (nearest fixed order) |
//! | `coeff.sum(axis=1)` | pairwise `f64`, block 8 | [`pairwise_f64`] |
//! | `pos.mean(axis=0)` | **sequential** `f32` per column | [`column_means_f32`] |
//! | `np.einsum("ij,ij->i", a, a)` | sequential `f32`, `((a+b)+c)` | [`dot3_f32`] |
//!
//! The `mean(axis=0)` / `mean()` asymmetry is not a typo: a contiguous one-dimensional
//! reduction runs numpy's pairwise sum, while a reduction along the *outer* axis of a
//! `(n, 3)` array runs the plain `add` inner loop and is therefore sequential. The same
//! holds in `f64`.

/// numpy's `PW_BLOCKSIZE` (`numpy/core/src/umath/loops_utils.h.src`): the widest run a
/// single unrolled pass covers before `pairwise_*` splits the array in half.
const BLOCK: usize = 128;

/// numpy's `pairwise_sum_FLOAT` / `pairwise_sum_DOUBLE`.
///
/// **Block of eight independent lanes, combined as a balanced tree, then a sequential
/// tail.** Reproduced because a sequential `f32` sum is a different number: the two differ
/// by an ulp on most inputs, and `f32` has 24 bits, so an ulp is a relative `6e-8` — enough
/// to move the adaptive speed and from there every position.
pub(super) fn pairwise_f32(values: &[f32]) -> f32 {
    if values.len() < 8 {
        return values.iter().fold(0.0f32, |acc, &v| acc + v);
    }
    if values.len() <= BLOCK {
        return lanes_f32(values);
    }
    let mut split = values.len() / 2;
    split -= split % 8;
    pairwise_f32(&values[..split]) + pairwise_f32(&values[split..])
}

/// [`pairwise_f32`]'s `f64` twin, for `coeff.sum(axis=1)`.
pub(super) fn pairwise_f64(values: &[f64]) -> f64 {
    if values.len() < 8 {
        return values.iter().fold(0.0f64, |acc, &v| acc + v);
    }
    if values.len() <= BLOCK {
        return lanes_f64(values);
    }
    let mut split = values.len() / 2;
    split -= split % 8;
    pairwise_f64(&values[..split]) + pairwise_f64(&values[split..])
}

/// The unrolled body both pairwise sums share: eight accumulators, a whole number of
/// eight-element groups, then the `0..8` remainder.
fn lanes_f32(values: &[f32]) -> f32 {
    let mut lanes = [0.0f32; 8];
    lanes.copy_from_slice(&values[..8]);
    let stop = values.len() - values.len() % 8;
    for (i, chunk) in values[8..stop].chunks_exact(8).enumerate() {
        for (lane, &v) in lanes.iter_mut().zip(chunk) {
            *lane += v;
        }
        let _ = i;
    }
    let tree = ((lanes[0] + lanes[1]) + (lanes[2] + lanes[3]))
        + ((lanes[4] + lanes[5]) + (lanes[6] + lanes[7]));
    tree + values[stop..].iter().sum::<f32>()
}

/// [`lanes_f32`]'s `f64` twin.
fn lanes_f64(values: &[f64]) -> f64 {
    let mut lanes = [0.0f64; 8];
    lanes.copy_from_slice(&values[..8]);
    let stop = values.len() - values.len() % 8;
    for chunk in values[8..stop].chunks_exact(8) {
        for (lane, &v) in lanes.iter_mut().zip(chunk) {
            *lane += v;
        }
    }
    let tree = ((lanes[0] + lanes[1]) + (lanes[2] + lanes[3]))
        + ((lanes[4] + lanes[5]) + (lanes[6] + lanes[7]));
    tree + values[stop..].iter().sum::<f64>()
}

/// `arr.mean(axis=0)` on an `(n, 3)` `f32` array — **sequential**, per column.
///
/// This is the one place the order is *not* pairwise: the reduction runs along the outer
/// axis of a C-contiguous array, so numpy's `add` inner loop walks the rows and the sum is
/// a plain left-to-right accumulation. Measured: `seq` matches at every `n` tried and
/// `pairwise` does not from `n = 77` on.
pub(super) fn column_means_f32(values: &[f32], n: usize) -> [f32; 3] {
    let mut sums = [0.0f32; 3];
    for row in values.chunks_exact(3).take(n) {
        for (sum, &v) in sums.iter_mut().zip(row) {
            *sum += v;
        }
    }
    let count = n as f32;
    [sums[0] / count, sums[1] / count, sums[2] / count]
}

/// [`column_means_f32`]'s `f64` twin, for the rescale in `_fa2_rescale`.
pub(super) fn column_means_f64(values: &[f64], n: usize) -> [f64; 3] {
    let mut sums = [0.0f64; 3];
    for row in values.chunks_exact(3).take(n) {
        for (sum, &v) in sums.iter_mut().zip(row) {
            *sum += v;
        }
    }
    let count = n as f64;
    [sums[0] / count, sums[1] / count, sums[2] / count]
}

/// `np.einsum("ij,ij->i", row, row)` in `f32`: three products, summed `((a+b)+c)`.
///
/// **Not the `f64` answer narrowed.** Measured: the `f32` association matches and the
/// `f64`-then-narrow one does not (`row 1` of the probe: `6.374669551849365` against
/// `6.374669671058655`), because `einsum` accumulates in the array's own dtype.
pub(super) fn dot3_f32(row: &[f32]) -> f32 {
    dot3_pair_f32(row, row)
}

/// The same three-term `f32` dot between two rows — `cross = t @ sources.T`, which the
/// reference reaches through a BLAS `sgemm` with an inner dimension of three.
///
/// **The reference's kernel is an FMA chain; this is the sequential `f32` dot.** Measured
/// on 5929 cells: 0 differ from `acc = fma(a_k, b_k, acc)`, 2275 differ from this. The
/// difference does not reach the finished layout (see `pair_force.rs`), and an FMA is not
/// a portable primitive here (D1: no `mul_add`, whose result is target-dependent).
pub(super) fn dot3_pair_f32(a: &[f32], b: &[f32]) -> f32 {
    let first = a[0] * b[0];
    let second = a[1] * b[1];
    (first + second) + a[2] * b[2]
}

/// [`dot3_f32`]'s `f64` twin, for `norm = sqrt(einsum(disp, disp))` on the `f64`
/// displacement.
pub(super) fn dot3_f64(row: &[f64]) -> f64 {
    let a = row[0] * row[0];
    let b = row[1] * row[1];
    (a + b) + row[2] * row[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairwise_f32_matches_the_block_of_eight_association() {
        // 10 values: the 8-lane body plus a two-value sequential tail, and the
        // association is what a sequential loop would get wrong.
        let values: Vec<f32> = (0..10).map(|i| 1.0e8 + f32::from(i as u8)).collect();
        let mut lanes = [0.0f32; 8];
        lanes.copy_from_slice(&values[..8]);
        for chunk in values[8..8].chunks_exact(8) {
            for (lane, &v) in lanes.iter_mut().zip(chunk) {
                *lane += v;
            }
        }
        let tree = ((lanes[0] + lanes[1]) + (lanes[2] + lanes[3]))
            + ((lanes[4] + lanes[5]) + (lanes[6] + lanes[7]));
        assert_eq!(pairwise_f32(&values), tree + values[8] + values[9]);
        // And it is *not* the sequential sum, which is the whole reason for the block.
        let sequential = values.iter().fold(0.0f32, |acc, &v| acc + v);
        assert_ne!(pairwise_f32(&values), sequential);
    }

    /// Above `BLOCK` numpy splits the array in half, and the split point is rounded
    /// **down to a multiple of eight** — an odd split would put a different number of
    /// values in the unrolled body and change the answer.
    #[test]
    fn pairwise_sums_above_the_block_split_on_a_multiple_of_eight() {
        let values: Vec<f32> = (0..200).map(|i| 1.0e8 + f32::from(i as u8)).collect();
        let mut split = values.len() / 2;
        split -= split % 8;
        assert_eq!(split, 96);
        let want = pairwise_f32(&values[..96]) + pairwise_f32(&values[96..]);
        assert_eq!(pairwise_f32(&values), want);
        let mut other = values.len() / 2;
        assert_ne!(other, split);
    }

    /// A one-dimensional reduction is pairwise; the same numbers reduced along the outer
    /// axis of an `(n, 3)` array are sequential. Both are in the reference, and confusing
    /// them moves the mean by an ulp.
    #[test]
    fn a_contiguous_reduction_is_pairwise_and_an_outer_axis_one_is_sequential() {
        let column: Vec<f32> = (0..77).map(|i| 1.0e7 + f32::from((i % 5) as u8)).collect();
        let flat: Vec<f32> = column.iter().copied().cycle().take(3 * 77).collect();
        assert_eq!(
            column_means_f32(&flat, 77)[0],
            pairwise_f32(&column) / 77.0f32
        );
        let mut sequential = 0.0f32;
        for &v in &column {
            sequential += v;
        }
        assert_ne!(column_means_f32(&flat, 77)[0], sequential / 77.0f32);
    }

    #[test]
    fn dot3_is_the_f32_association_not_the_f64_answer_narrowed() {
        let row = [6.0f32, 2.0, 3.0];
        let exact = (f64::from(row[0]) * f64::from(row[0])
            + f64::from(row[1]) * f64::from(row[1]))
            + f64::from(row[2]) * f64::from(row[2]);
        assert_eq!(dot3_f32(&row), 49.0f32);
        assert_ne!(dot3_f32(&row), exact as f32);
        let wide: [f64; 3] = [6.0, 2.0, 3.0];
        assert_eq!(dot3_f64(&wide), exact);
    }

    #[test]
    fn the_reductions_are_bit_identical_twice_over() {
        let values: Vec<f32> = (0..300).map(|i| f32::from((i % 17) as u8) * 0.5).collect();
        assert_eq!(pairwise_f32(&values), pairwise_f32(&values));
        let doubles: Vec<f64> = values.iter().map(|&v| f64::from(v)).collect();
        assert_eq!(pairwise_f64(&doubles), pairwise_f64(&doubles));
    }
}

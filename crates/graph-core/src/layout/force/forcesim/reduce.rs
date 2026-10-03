//! The reductions the reference's FA2 kernel performs, each pinned to the order numpy
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
//! | `pos.mean(axis=0)`, `arr.mean(axis=0)` | **sequential**, per column | [`column_means_f32`], [`column_means_f64`] |
//! | `np.einsum("ij,ij->i", a, a)` | sequential `f32`, `((a+b)+c)` | [`dot3_f32`], [`dot3_pair_f32`] |
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
/// **Block of eight independent lanes, a balanced tree over them, then a sequential tail.**
/// Reproduced because a sequential `f32` sum is a different number: the two differ by an
/// ULP on most inputs, and `f32` has 24 bits, so an ULP is a relative `6e-8` — enough to
/// move the adaptive speed and from there every position.
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
/// eight-element groups, a balanced tree over those accumulators, then the `0..8`
/// remainder.
///
/// **The remainder is folded into the tree one value at a time, not summed on its own and
/// then added.** numpy writes `res = res + a[i]` in a loop; `tree + (a[8] + a[9] + ...)` is
/// a different `f32`. On the fifteen `f32` values this kernel actually sums — measured —
/// the two are one ULP apart (`0x43808672` against `0x43808671`), and one ULP of
/// `total_swing` is one ULP of the adaptive speed and from there of every position.
fn lanes_f32(values: &[f32]) -> f32 {
    let mut lanes = [0.0f32; 8];
    lanes.copy_from_slice(&values[..8]);
    let stop = values.len() - values.len() % 8;
    for chunk in values[8..stop].as_chunks::<8>().0 {
        for (lane, &v) in lanes.iter_mut().zip(chunk) {
            *lane += v;
        }
    }
    let mut total = ((lanes[0] + lanes[1]) + (lanes[2] + lanes[3]))
        + ((lanes[4] + lanes[5]) + (lanes[6] + lanes[7]));
    for &v in &values[stop..] {
        total += v;
    }
    total
}

/// [`lanes_f32`]'s `f64` twin.
fn lanes_f64(values: &[f64]) -> f64 {
    let mut lanes = [0.0f64; 8];
    lanes.copy_from_slice(&values[..8]);
    let stop = values.len() - values.len() % 8;
    for chunk in values[8..stop].as_chunks::<8>().0 {
        for (lane, &v) in lanes.iter_mut().zip(chunk) {
            *lane += v;
        }
    }
    let mut total = ((lanes[0] + lanes[1]) + (lanes[2] + lanes[3]))
        + ((lanes[4] + lanes[5]) + (lanes[6] + lanes[7]));
    for &v in &values[stop..] {
        total += v;
    }
    total
}

/// `arr.mean(axis=0)` on an `(n, 3)` `f32` array — **sequential**, per column.
///
/// This is the one place the order is *not* pairwise: the reduction runs along the outer
/// axis of a C-contiguous array, so numpy's `add` inner loop walks the rows and the sum is
/// a plain left-to-right accumulation. Measured: `seq` matches at every `n` tried and
/// `pairwise` does not from `n = 77` on.
pub(super) fn column_means_f32(values: &[f32], n: usize) -> [f32; 3] {
    let mut sums = [0.0f32; 3];
    for row in values.as_chunks::<3>().0.iter().take(n) {
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
    for row in values.as_chunks::<3>().0.iter().take(n) {
        for (sum, &v) in sums.iter_mut().zip(row) {
            *sum += v;
        }
    }
    let count = n as f64;
    [sums[0] / count, sums[1] / count, sums[2] / count]
}

/// `np.einsum("ij,ij->i", row, row)` in `f32`: three products, summed `((a+b)+c)`.
///
/// **Not the `f64` answer narrowed.** `einsum` accumulates in the array's own dtype, and on
/// the probe's `row 1` the `f32` association gives `6.374669551849365` where the `f64`
/// answer narrowed gives `6.374669671058655`.
pub(super) fn dot3_f32(row: &[f32]) -> f32 {
    dot3_pair_f32(row, row)
}

/// The same three-term `f32` dot between two rows — `cross = t @ sources.T`, which the
/// reference reaches through a BLAS `sgemm` with an inner dimension of three.
///
/// **The reference's kernel is an FMA chain; this is the sequential `f32` dot.** Measured
/// on 5929 cells of `(n, 3) @ (3, n)`: 0 differ from `acc = fma(a_k, b_k, acc)`, 2275
/// differ from this. The difference does not reach the finished layout (see
/// `pair_force.rs`), and an FMA is not a portable primitive here (D1: no `mul_add`, whose
/// result is target-dependent).
pub(super) fn dot3_pair_f32(a: &[f32], b: &[f32]) -> f32 {
    let first = a[0] * b[0];
    let second = a[1] * b[1];
    (first + second) + a[2] * b[2]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fifteen `f32` values `total_swing` is actually reduced over at `tree-balanced`'s
    /// second iteration, as bits, with numpy's answer beside them.
    ///
    /// A measured vector rather than synthetic numbers: `1e8 + i` and `1e7 + i` are the
    /// obvious way to build a reduction whose order matters and they are *useless* here,
    /// because `f32` at `1e8` has a ULP of 8 and every one of those values rounds to the
    /// same `f32`. These fifteen are the ones that separated the tail folded into the tree
    /// from the tail summed on its own.
    const MEASURED: [u32; 15] = [
        0x41EB_D930,
        0x410C_ED93,
        0x40BF_B904,
        0x418E_FD17,
        0x41E8_5794,
        0x40D3_C252,
        0x40C6_F1B9,
        0x416E_D4ED,
        0x424A_ABFB,
        0x41A2_A046,
        0x4112_F274,
        0x41D3_9C1C,
        0x40F6_930C,
        0x40DD_C3EB,
        0x4186_9969,
    ];

    fn measured() -> Vec<f32> {
        MEASURED.iter().map(|&b| f32::from_bits(b)).collect()
    }

    #[test]
    fn pairwise_f32_is_numpys_answer_on_the_measured_fifteen() {
        assert_eq!(pairwise_f32(&measured()).to_bits(), 0x4380_8671);
    }

    /// The negative control for that vector: summing the remainder on its own and adding
    /// it to the tree is a different `f32`, by exactly one ULP. Without this the test above
    /// would also pass against the wrong association.
    #[test]
    fn the_remainder_is_folded_into_the_tree_and_not_added_to_it() {
        let values = measured();
        let mut lanes = [0.0f32; 8];
        lanes.copy_from_slice(&values[..8]);
        let tree = ((lanes[0] + lanes[1]) + (lanes[2] + lanes[3]))
            + ((lanes[4] + lanes[5]) + (lanes[6] + lanes[7]));
        let grouped = values[8..].iter().fold(0.0f32, |acc, &v| acc + v);
        assert_eq!(
            (tree + grouped).to_bits(),
            pairwise_f32(&values).to_bits() + 1,
            "the two associations are one ULP apart"
        );
    }

    /// Above `BLOCK` numpy splits the array in half, and the split point is rounded
    /// **down to a multiple of eight** — an odd split would put a different number of
    /// values in the unrolled body and change the answer.
    ///
    /// `n = 130`, not something smaller: `pairwise_*` only splits above `PW_BLOCKSIZE`, so
    /// a length under 128 never reaches this branch at all.
    #[test]
    fn pairwise_sums_above_the_block_split_on_a_multiple_of_eight() {
        let mut values = measured();
        while values.len() < 130 {
            values.extend(measured());
        }
        values.truncate(130);
        assert_eq!(values.len(), 130);
        let mut split = values.len() / 2;
        split -= split % 8;
        assert_eq!(split, 64);
        assert_eq!(
            pairwise_f32(&values),
            pairwise_f32(&values[..64]) + pairwise_f32(&values[64..])
        );
        assert_ne!(values.len() / 2, split);
    }

    /// A one-dimensional reduction is pairwise; the same numbers reduced along the outer
    /// axis of an `(n, 3)` array are sequential. Both orders are in this kernel, and
    /// confusing them moves the mean by an ULP.
    #[test]
    fn a_contiguous_reduction_is_pairwise_and_an_outer_axis_one_is_sequential() {
        let column: Vec<f32> = (0..77)
            .map(|i| 1.0e7 + f32::from((i % 5) as u8) + f32::from((i / 7) as u8))
            .collect();
        // An (n, 3) array whose *first* column is `column`, flat and C-order.
        let flat: Vec<f32> = column.iter().flat_map(|&v| [v, v + 1.0, v + 2.0]).collect();
        let sequential = column.iter().fold(0.0f32, |acc, &v| acc + v) / 77.0f32;
        assert_ne!(pairwise_f32(&column) / 77.0f32, sequential);
        assert_eq!(column_means_f32(&flat, 77)[0], sequential);
    }

    /// `einsum` accumulates in the array's own dtype and associates `((a+b)+c)`, which is
    /// not the `f64` answer narrowed. The three values are measured, and both sides are
    /// pinned so the test says which is which.
    #[test]
    fn dot3_is_the_f32_association_not_the_f64_answer_narrowed() {
        let row = [
            f32::from_bits(0x3C91_69F9),
            f32::from_bits(0x3AE0_ADC0),
            f32::from_bits(0xBC0D_7A4B),
        ];
        let exact = (f64::from(row[0]) * f64::from(row[0]) + f64::from(row[1]) * f64::from(row[1]))
            + f64::from(row[2]) * f64::from(row[2]);
        assert_eq!(dot3_f32(&row).to_bits(), 0x39CD_D4CA);
        assert_eq!(exact as f32, f32::from_bits(0x39CD_D4C9));
        assert_eq!(dot3_f32(&row), dot3_pair_f32(&row, &row));
    }

    #[test]
    fn the_reductions_are_bit_identical_twice_over() {
        let values = measured();
        assert_eq!(pairwise_f32(&values), pairwise_f32(&values));
        let doubles: Vec<f64> = values.iter().map(|&v| f64::from(v)).collect();
        assert_eq!(pairwise_f64(&doubles), pairwise_f64(&doubles));
        let flat: Vec<f32> = values.iter().copied().cycle().take(45).collect();
        assert_eq!(column_means_f32(&flat, 15), column_means_f32(&flat, 15));
    }
}

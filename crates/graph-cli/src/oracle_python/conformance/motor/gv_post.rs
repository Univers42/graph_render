//! SciGraphs' own layer over an engine's points, as one function, so the motor arm and the
//! reference arm apply the identical transform and every Graphviz row is measured in one
//! unit system instead of two.
//!
//! **This belongs to the conformance arm and not to `twopi.rs` or `squarify.rs`,** which is
//! where `docs/measurements/scigraphs-conformance.md`'s repair 2 put it. A Graphviz port in
//! `graph-core` is gated against Graphviz 16.1.0's own output (its registry `oracle`), and
//! this transform is SciGraphs' layer *below* that output: a port that applied it would
//! disagree with the engine by construction, and the engine is the thing the port is measured
//! against. The user decided this on 2026-09-30 and nothing under `crates/graph-core` moves
//! for it.

/// `dims` for every row this arm runs: `dims = min(3, raw.shape[1])` (`yifan_hu.py:315`) over
/// an engine that writes two columns, with `graphviz_dim` at its default `"2"`
/// (`yifan_hu.py:314`). `YIFAN_HU` asks for `"2Z"` (`yifan_hu.py:357`), which changes the z
/// column at `:327-334` and not this number.
pub const GRAPHVIZ_DIMS: usize = 2;

/// SciGraphs' lines 318 to 325 over `points`: centre on the mean, divide by the largest extent
/// of the first `dims` columns, write those columns into a zeroed `(n, 3)` and multiply by
/// `scale`.
///
/// **The `np.zeros` at `:323` is a line of the convention and not scaffolding.** It is why a 2D
/// engine's z comes out `0.0` rather than the z it was handed, and a version of this function
/// that passed the third column through would put a z in the file that SciGraphs never writes.
pub fn scigraphs_graphviz_post(points: &[[f64; 3]], dims: usize, scale: f64) -> Vec<[f64; 3]> {
    if points.is_empty() {
        return Vec::new();
    }
    let axes = dims.min(3);
    let mean = mean_over(points, axes);
    let centred: Vec<[f64; 3]> = points.iter().map(|p| shifted(*p, &mean)).collect();
    let extent = (0..axes)
        .map(|c| column_range(&centred, c))
        .fold(0.0, f64::max);
    let divisor = if extent > 0.0 { extent } else { 1.0 };
    centred
        .iter()
        .map(|p| written(*p, axes, divisor, scale))
        .collect()
}

/// `raw.mean(axis=0)` over the first `axes` columns: numpy's summation, then the division
/// `_methods._mean` does once over the total.
fn mean_over(points: &[[f64; 3]], axes: usize) -> Vec<f64> {
    let count = points.len() as f64;
    (0..axes)
        .map(|c| {
            let column: Vec<f64> = points.iter().map(|p| p[c]).collect();
            numpy_pairwise_sum(&column) / count
        })
        .collect()
}

/// `raw - raw.mean(axis=0)`, over the columns the mean covers; the rest become `0.0`.
fn shifted(point: [f64; 3], mean: &[f64]) -> [f64; 3] {
    std::array::from_fn(|c| {
        if c < mean.len() {
            point[c] - mean[c]
        } else {
            0.0
        }
    })
}

/// `raw.max(axis=0) - raw.min(axis=0)` for one column.
fn column_range(points: &[[f64; 3]], c: usize) -> f64 {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for p in points {
        lo = lo.min(p[c]);
        hi = hi.max(p[c]);
    }
    hi - lo
}

/// `positions[:, :dims] = raw[:, :dims]` into a zeroed `(n, 3)`, then `positions *= scale`.
fn written(centred: [f64; 3], axes: usize, extent: f64, scale: f64) -> [f64; 3] {
    std::array::from_fn(|c| {
        if c < axes {
            centred[c] / extent * scale
        } else {
            0.0
        }
    })
}

/// numpy's own summation for `float64`, which is what `raw.mean(axis=0)` reduces with: a
/// plain left-to-right sum below eight elements, eight accumulators up to `PW_BLOCKSIZE =
/// 128`, and above that a split in two at an eight-aligned midpoint.
///
/// **The order is the point, and how much it is worth is measured.** A left-to-right sum of
/// the same `n` values lands several ULPs away on the probes in [`tests`]: at `n = 300` the
/// mean is `0x1.999999999999ap-7` against a naive `0x1.df04444444444p-7`. Both arms of a
/// Graphviz row reduce the *same* mean from *different* points, so any order one arm does not
/// share is an offset the centring never removes. Which order this is, is measured against
/// numpy 2.3.3 by [`tests::the_sum_here_is_numpys_own_order`] rather than asserted here.
fn numpy_pairwise_sum(values: &[f64]) -> f64 {
    const PW_BLOCKSIZE: usize = 128;
    let n = values.len();
    if n < 8 {
        return left_to_right(values);
    }
    if n > PW_BLOCKSIZE {
        let split = (n / 2) & !7;
        return numpy_pairwise_sum(&values[..split]) + numpy_pairwise_sum(&values[split..]);
    }
    let mut acc = [0.0f64; 8];
    acc.copy_from_slice(&values[..8]);
    let mut i = 8;
    while i < n - n % 8 {
        for (k, a) in acc.iter_mut().enumerate() {
            *a += values[i + k];
        }
        i += 8;
    }
    let folded = ((acc[0] + acc[1]) + (acc[2] + acc[3])) + ((acc[4] + acc[5]) + (acc[6] + acc[7]));
    values[i..].iter().fold(folded, |total, v| total + v)
}

/// The reduction this function exists **not** to be: numpy's `n < 8` branch, kept as its own
/// named function so the negative control in [`tests`] is the same code the reference runs
/// rather than a second transcription of it.
fn left_to_right(values: &[f64]) -> f64 {
    let mut total = 0.0;
    for v in values {
        total += v;
    }
    total
}

#[cfg(test)]
mod tests;

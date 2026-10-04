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

/// `raw.mean(axis=0)` over the first `axes` columns: a left-to-right sum down each column, then
/// the division `_methods._mean` does once over the total.
///
/// Ponytail: the order is **left to right, not a pairwise sum**, and the reason is that numpy's
/// pairwise reduction only runs along the *contiguous* axis. `raw` is `(n, 3)` C-contiguous, so
/// axis 0 is the strided one and `mean(axis=0)` walks each column as a flat sequence; the eight
/// accumulators and the split above `PW_BLOCKSIZE` belong to the contiguous axis and never run
/// here. Measured in the oracle image on `(n, 2)` C-contiguous arrays at
/// n = 1, 2, 3, 5, 8, 9, 16, 17, 33, 64, 127, 128, 129, 300: left-to-right equals numpy at every
/// length and on both columns, and a pairwise sum of the same values differs at every n >= 8 —
/// pinned by `tests::the_mean_of_an_n_by_2_array_is_the_left_to_right_sum`, which pastes numpy's
/// own hex. The caveat that is left: a Fortran-ordered `raw` would restore the pairwise sum, and
/// `scigraphs_utils` is a C++ extension with no source on disk to read its allocation from.
fn mean_over(points: &[[f64; 3]], axes: usize) -> Vec<f64> {
    let count = points.len() as f64;
    (0..axes)
        .map(|c| {
            let column: Vec<f64> = points.iter().map(|p| p[c]).collect();
            left_to_right(&column) / count
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

/// The reduction `mean_over` runs: numpy's `mean` over a strided axis, which is a plain
/// left-to-right sum of one column and then a single division.
fn left_to_right(values: &[f64]) -> f64 {
    let mut total = 0.0;
    for v in values {
        total += v;
    }
    total
}

#[cfg(test)]
mod tests;

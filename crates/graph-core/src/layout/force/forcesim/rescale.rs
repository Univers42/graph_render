//! `_fa2_rescale` (`forceatlas.py:47-56`), the last thing the reference does before it
//! hands the layout back.
//!
//! ```text
//! arr  = arr - arr.mean(axis=0)          f64, sequential per column
//! span = float(np.abs(arr).max())         f64, order-independent
//! if span > 1e-9: arr = arr * (scale / span)
//! ```
//!
//! **Two `f64` details.** `arr` arrives as `np.asarray(sim.positions, dtype=np.float64)`
//! (`forceatlas.py:147`), so the whole rescale is `f64` over `f32` inputs — the narrowing to
//! `f32` for the wire happens once, in [`super::in_space`]. And `mean(axis=0)` on an
//! `(n, 3)` array is a **sequential** `f64` sum per column, not the pairwise order a
//! contiguous reduction uses; measured in `ge-python-oracle`, sequential matches and
//! pairwise does not from `n = 77` on.
//!
//! The `span > 1e-9` guard is the reference's own: a layout that has collapsed to a point,
//! or a graph with no nodes, is returned unchanged rather than divided by zero.

use super::reduce::column_means_f64;

/// Below this the layout is returned as it is (`forceatlas.py:54`).
const MIN_SPAN: f64 = 1e-9;

/// Centre on the origin and fit the widest axis to `scale`.
pub(super) fn rescale(pos: &[f32], scale: f64) -> Vec<f64> {
    let mut arr: Vec<f64> = pos.iter().map(|&v| f64::from(v)).collect();
    let means = column_means_f64(&arr, arr.len() / 3);
    for (i, value) in arr.iter_mut().enumerate() {
        *value -= means[i % 3];
    }
    let span = arr.iter().fold(0.0f64, |widest, v| widest.max(v.abs()));
    if span > MIN_SPAN {
        let fit = scale / span;
        for value in arr.iter_mut() {
            *value *= fit;
        }
    }
    arr
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_widest_coordinate_becomes_scale_and_the_centre_becomes_zero() {
        let pos = [-1.0f32, 0.0, 0.0, 3.0, 0.0, 0.0];
        let out = rescale(&pos, 5.0);
        // mean = 1, so the columns are -2 and +2; the widest is 2, fitted to 5.
        assert_eq!(out, vec![-5.0, 0.0, 0.0, 5.0, 0.0, 0.0]);
        assert!((column_means_f64(&out, 2)[0]).abs() < 1e-12);
    }

    #[test]
    fn the_span_is_the_widest_over_all_three_columns_not_the_widest_column() {
        // x spans 1, y spans 10: the y extent is what fits to `scale`.
        let pos = [0.0f32, -5.0, 0.0, 1.0, 5.0, 0.0];
        let out = rescale(&pos, 2.0);
        let widest = out.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        assert!((widest - 2.0).abs() < 1e-12, "widest {widest}");
    }

    /// A collapsed layout divides by nothing: the reference's `span > 1e-9` guard returns it
    /// centred but unscaled.
    #[test]
    fn a_collapsed_layout_is_returned_unscaled() {
        let pos = [1.0f32, 2.0, 3.0, 1.0, 2.0, 3.0];
        let out = rescale(&pos, 5.0);
        assert!(out.iter().all(|&v| v == 0.0), "{out:?}");
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn an_empty_layout_stays_empty() {
        assert!(rescale(&[], 5.0).is_empty());
    }
}

//! The two order statistics a batch table adds on top of [`crate::bench::campaign::median`]:
//! the 95th percentile and the maximum. Median is the one both arms already had, and the three
//! of them are computed one way in this module and in `harness/wasm-stream-bench.mjs`, or the
//! native and wasm columns would not be comparable.
//!
//! Caveat: ten batches make a thin p95. The percentile is interpolated between order statistics
//! (the `R7` rule, so p95 of ten samples is strictly below the maximum and says something the
//! maximum does not), but with `n = 10` it is one interpolated value between the ninth and tenth
//! smallest sum, not a tail anyone should read as one. It is printed because the contract asks
//! for it, and the report calls it thin.

/// The `q` quantile of `samples`, interpolating linearly between order statistics.
///
/// An empty list is `0.0`, as [`crate::bench::campaign::median`]'s is: a table row is never
/// missing because the sample list was, and a `NaN` would print as one.
pub fn quantile(mut samples: Vec<f64>, q: f64) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    samples.sort_by(f64::total_cmp);
    let at = q.clamp(0.0, 1.0) * (samples.len() - 1) as f64;
    let low = at.floor() as usize;
    let high = at.ceil() as usize;
    samples[low] + (samples[high] - samples[low]) * (at - low as f64)
}

/// The 95th percentile: [`quantile`] at `0.95`.
pub fn p95(samples: Vec<f64>) -> f64 {
    quantile(samples, 0.95)
}

/// The largest sample; `0.0` for an empty list, as [`quantile`]'s.
pub fn max(samples: &[f64]) -> f64 {
    samples.iter().fold(0.0_f64, |hi, &s| hi.max(s))
}

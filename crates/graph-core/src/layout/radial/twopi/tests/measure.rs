//! The row `docs/measurements/fix-tree-twopi.md` quotes for L-01: time and peak heap on an
//! **edgeless** graph at 2 000, 16 000 and 128 000 nodes, the shape whose component count
//! equals its node count.
//!
//! Printed, never asserted. The assertion that pins the finding is `tests/alloc.rs`'s
//! allocation count, which is the same fact measured without a clock in it; a timing is
//! what the numbers here are for, and only a measurement belongs on the page that quotes
//! them.
//!
//! Run alone, in release, or the time is the harness's and not the layout's:
//! `cargo test --release -p graph-core --lib layout::radial::twopi::tests::measure -- \
//! --ignored --exact --nocapture`

use super::alloc::edgeless;
use std::time::Instant;

/// The sizes `docs/measurements/fix-tree-twopi.md` quotes, and the sizes an O(n^2) run is
/// most obviously quadratic at.
const SIZES: [u32; 3] = [2_000, 16_000, 128_000];

#[test]
#[ignore = "a measurement, not a check: run alone with --release -- --ignored --nocapture"]
fn an_edgeless_graph_measures_flat_in_time_and_in_bytes() {
    for count in SIZES {
        let start = Instant::now();
        let heap = edgeless(count);
        let elapsed = start.elapsed();
        println!(
            "| {count} | {:.2} ms | {} B | {} |",
            elapsed.as_secs_f64() * 1e3,
            heap.peak,
            heap.calls
        );
    }
}

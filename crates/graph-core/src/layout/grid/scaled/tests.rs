//! `Grid::run_scaled`'s own tests, split out of `scaled.rs` by the house file limit.
//!
//! They are about the SciGraphs placement — the `f64` multiply-then-divide, the two scale
//! rules and the boundary where the `f32` range runs out — where the registered lattice's
//! tests live in `../tests.rs`.

use super::*;
use crate::exec::Serial;
use crate::layout::coords::probe;

/// The case the conformance row rests on, worked out on paper and not from the kernel:
/// `n = 5` gives `cols = 3`, so `x = 0, 5/3, 10/3, 0, 5/3` and `y = 0, 0, 0, 5/3, 5/3`
/// — `y` is `(i / 3) * 5 / 3`, so row 1 sits at `5/3` and there is no row 2.
#[test]
fn the_placement_is_the_reference_one_at_scale_five() {
    assert_eq!(
        layout(5, 5.0),
        vec![
            (0.0, 0.0),
            (5.0 / 3.0, 0.0),
            (10.0 / 3.0, 0.0),
            (0.0, 5.0 / 3.0),
            (5.0 / 3.0, 5.0 / 3.0),
        ]
    );
}

/// The reference formula over every node count worth holding, against the kernel the
/// threaded arms run — the expectation is numpy's arithmetic and the answer is the
/// gather's, so the two cannot be one drawing.
#[test]
fn every_node_matches_the_reference_formula() {
    for n in [0, 1, 2, 3, 4, 5, 9, 10, 16, 17, 50, 64, 100, 601, 1000] {
        assert_eq!(layout(n, 5.0), reference(n, 5.0), "n = {n}");
    }
}

/// The same per-node gather as the parent's byte-identity test, and the same claim: a
/// coordinate is a function of the node's own index, so a width can only get a range
/// boundary wrong.
#[test]
fn the_gather_is_the_same_bytes_at_every_worker_count() {
    for n in [0, 1, 5, 16, 1000] {
        let want = reference(n, 5.0);
        for workers in [1, 2, 3, 4, 7] {
            assert_eq!(
                scaled_layout(n, 5.0, workers),
                want,
                "n = {n}, workers = {workers}"
            );
        }
    }
}

/// **The negative control for the `f64` in [`ScaledLattice::at`].** A `f32`
/// `GridParams::spacing` of `fl32(5 / 9)` and the registered kernel's one product puts
/// node 3 a whole ULP off the reference, so folding this kernel into the `f32` one to
/// save a struct would cost the row. Chosen because `3 · fl32(5/9)` is *representable* in
/// `f32` (24 significand bits, no rounding at all), so the difference below is the two
/// formulas and not a rounding direction.
///
/// **It runs the kernel.** `n = 65` gives `cols = 9`, so node 3 is column 3 of row 0 and
/// its `x` is the case; the first assertion is the gather's own bits, so a kernel folded
/// onto an `f32` pitch fails here rather than passing a comparison of two literals.
#[test]
fn a_f32_pitch_would_not_have_reached_these_bits() {
    let got = layout(65, 5.0);
    assert_eq!(got[3], (5.0_f32 / 3.0, 0.0), "the kernel's own bits");
    let pitch = 5.0_f32 / 9.0;
    assert_eq!(
        3.0 * pitch,
        1.666_666_7,
        "the f32 pitch the parent would have used"
    );
    assert_ne!(
        got[3],
        (3.0 * pitch, 0.0),
        "a whole ULP: two formulas, not a rounding"
    );
}

/// The rule that keeps a non-finite coordinate out, at the scale the review named: `n = 4`
/// gives `cols = 2`, so the widest coordinate is `1e39 / 2 = 5e38`, past `f32::MAX`, and
/// the `as f32` would write `inf`. The public entry point is refused, not the helper alone.
#[test]
fn a_scale_that_does_not_fit_the_f32_range_is_refused() {
    let want = StageError::Param {
        name: "scale",
        rule: "widest coordinate inside the f32 range",
    };
    assert_eq!(scaled_cells(4, 1e39).expect_err("refused"), want);
    assert_eq!(
        Grid::run_scaled(&probe::graph(4, &[]), 1e39, &Serial, 1).expect_err("refused"),
        want
    );
}

/// The other side of the same comparison, so the rule is a bound and not a veto: the
/// largest scale whose widest coordinate *is* `f32::MAX` is placed, at exactly that value.
#[test]
fn the_largest_scale_the_f32_range_holds_is_placed() {
    let scale = 2.0 * f64::from(f32::MAX);
    assert!(scaled_cells(4, scale).is_ok());
    assert_eq!(
        layout(4, scale),
        vec![
            (0.0, 0.0),
            (f32::MAX, 0.0),
            (0.0, f32::MAX),
            (f32::MAX, f32::MAX),
        ]
    );
}

#[test]
fn a_scale_that_is_not_finite_and_positive_is_refused() {
    for scale in [0.0, -0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            scaled_cells(4, scale).expect_err("refused"),
            StageError::Param {
                name: "scale",
                rule: "finite and above 0"
            },
            "{scale}"
        );
    }
    assert!(scaled_cells(4, f64::MIN_POSITIVE).is_ok());
}

/// `basic.py:16-17` as a function: `cols = ceil(sqrt(n))`, and node `i`'s cell is
/// `(i % cols, i / cols)`, each coordinate `(k * scale) / cols` in `f64`.
///
/// **`cols` is counted here, not taken from `super::dimensions`.** The helper used to
/// ask the production `dimensions` for it, which made the whole expectation a function
/// of the code it was checking: a regression in `dimensions` was reproduced on both
/// sides and this test stayed green. `columns_by_hand` below is the independent
/// oracle, and its multiplication is in `u64` because `65536 * 65536` does not fit a
/// `u32`.
fn reference(n: u32, scale: f64) -> Vec<(f32, f32)> {
    let cols = columns_by_hand(n);
    (0..n)
        .map(|i| (cell(i % cols, scale, cols), cell(i / cols, scale, cols)))
        .collect()
}

/// `ceil(sqrt(n))` counted up one step at a time, from `basic.py:16` and not from
/// `dimensions`' `isqrt`. `n = 0` has no columns at all, which is the kernel's own
/// answer and the only value that makes `% cols` legal.
fn columns_by_hand(n: u32) -> u32 {
    if n == 0 {
        return 0;
    }
    let mut cols = 1u32;
    while u64::from(cols) * u64::from(cols) < u64::from(n) {
        cols += 1;
    }
    cols
}

/// `(k * scale) / g`, multiply first: that is numpy's order and the only order the
/// reference uses.
fn cell(k: u32, scale: f64, g: u32) -> f32 {
    (k as f64 * scale / g as f64) as f32
}

/// `Grid::run_scaled` end to end, the way the conformance arm calls it.
fn layout(n: u32, scale: f64) -> Vec<(f32, f32)> {
    scaled_layout(n, scale, 1)
}

/// The same run with the worker count spelled out.
fn scaled_layout(n: u32, scale: f64, workers: u32) -> Vec<(f32, f32)> {
    let topology = probe::graph(n, &[]);
    let got =
        Grid::run_scaled(&topology, scale, &Serial, workers).expect("a scale the rule allows");
    let Geometry {
        nodes: NodeGeometry::Point { x, y },
        ..
    } = got
    else {
        panic!("point nodes");
    };
    x.into_iter().zip(y).collect()
}

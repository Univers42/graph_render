//! The reference values, pasted from numpy. **Every constant below came out of
//! `ge-python-oracle` running numpy 2.3.3 on the four lines at `yifan_hu.py:318-325`, not out
//! of the code under test.** A constant copied from the function it is meant to check cannot
//! fail, and these are the constants that decide whether a Graphviz row is `bitwise` or
//! `shape`.

use super::super::super::{ROWS, SCALE};
use super::super::{run, run_row};
use super::{GRAPHVIZ_DIMS, left_to_right, numpy_pairwise_sum, scigraphs_graphviz_post};

/// numpy's answer for three hand-written points at `dims = 2, scale = 5.0`.
///
/// The input is chosen so every step is inexact: the mean's `y` is `4.0` over a `4.5` and a
/// `9.75`, so `raw - mean` is exact while `raw / extent` and the multiply are not, and the
/// extent is the `y` range rather than the `x` one, so a swapped `dims` fails here.
const THREE: [&str; 9] = [
    "aaaaaaaaaaaaffbf", // -0x1.faaaaaaaaaaaap+0
    "5655555555d504c0", // -0x1.4d55555555556p+1
    "0000000000000000",
    "000000000000f93f", //  0x1.9000000000000p+0
    "aaaaaaaaaaaaca3f", //  0x1.aaaaaaaaaaaaap-3
    "0000000000000000",
    "aaaaaaaaaaaada3f", //  0x1.aaaaaaaaaaaaap-2
    "abaaaaaaaa2a0340", //  0x1.32aaaaaaaaaabp+1
    "0000000000000000",
];

#[test]
fn three_points_land_where_numpy_puts_them() {
    let points = [[1.5, -2.25, 0.0], [10.0, 4.5, 0.0], [7.25, 9.75, 0.0]];
    assert_eq!(
        bits(&scigraphs_graphviz_post(&points, GRAPHVIZ_DIMS, SCALE)),
        THREE
    );
}

/// The same three points with a `z` on each: `positions = np.zeros((num_nodes, 3))`
/// (`yifan_hu.py:323`) drops the third column, so numpy's answer is the one above again. A
/// transform that passed `z` through would write `0x1.2aaaaaaaaaaabp-1` in that slot, which is
/// `c000000000000040` and is nowhere in the constant.
#[test]
fn a_third_column_is_dropped_rather_than_moved() {
    let points = [[1.5, -2.25, 7.0], [10.0, 4.5, -3.0], [7.25, 9.75, 11.0]];
    assert_eq!(
        bits(&scigraphs_graphviz_post(&points, GRAPHVIZ_DIMS, SCALE)),
        THREE
    );
}

/// `raw / (extent if extent > 0 else 1.0)` (`yifan_hu.py:321`): three coincident points have
/// no extent, and numpy writes nine zeros rather than nine NaNs.
#[test]
fn a_layout_with_no_extent_is_all_zeros_and_not_nan() {
    let points = [[4.0, 4.0, 0.0]; 3];
    assert_eq!(
        bits(&scigraphs_graphviz_post(&points, GRAPHVIZ_DIMS, SCALE)),
        ["0000000000000000"; 9]
    );
    assert_eq!(scigraphs_graphviz_post(&[], GRAPHVIZ_DIMS, SCALE).len(), 0);
}

/// The summation order, against numpy's own, over the six lengths that bracket every branch
/// of its reduction: below eight, the eight-accumulator block, its non-multiple-of-eight
/// tail, and the split above `PW_BLOCKSIZE = 128`.
///
/// **The left-to-right column is the negative control, and it has to differ at every length.**
/// It is [`super::left_to_right`] — the same function the `n < 8` branch runs — so if it ever
/// agreed with numpy here, this test would be pinning a sum order that does not exist and the
/// reduction would be untested.
#[test]
fn the_sum_here_is_numpys_own_order() {
    const NUMPY: [(usize, &str); 6] = [
        (9, "9c6e3bce30b20442"),
        (16, "20ff718d13ab94c2"),
        (17, "2dd1c30181285f42"),
        (33, "b04d163c7b14b942"),
        (129, "75453d63421ac741"),
        (300, "9a9999999999893f"),
    ];
    for (n, mean) in NUMPY {
        let values = probe(n);
        let ours = numpy_pairwise_sum(&values) / n as f64;
        assert_eq!(bits_of(ours.to_bits()), *mean, "n = {n}: not numpy's mean");
        let control = left_to_right(&values) / n as f64;
        assert_ne!(
            control.to_bits(),
            ours.to_bits(),
            "n = {n}: the control agreed, so the order is not load-bearing"
        );
    }
}

/// Alternating signs against ten-to-the-fifteen magnitudes: cancellation heavy enough that
/// the order of the additions shows in the mean, and every term exactly representable so the
/// probe is the same array on both sides.
fn probe(n: usize) -> Vec<f64> {
    const MAGNITUDE: [f64; 5] = [1e15, 1e14, 1e13, 1e12, 1e11];
    const SIGN: [f64; 2] = [1.0, -1.0];
    (0..n)
        .map(|i| SIGN[i % 2] * MAGNITUDE[i % 5] + 1.0 / (i + 1) as f64)
        .collect()
}

/// The end-to-end property the eight Graphviz rows are gated on: their motor arm comes out
/// centred on zero with its widest extent at `scale`, where the raw port output is neither.
///
/// **The raw column is the control.** A `twopi` port already centred at the origin would make
/// the centring half of this test vacuous, so the assertion is that the raw points are *not*
/// what the row writes — a `max_gap` of 303 against the engine, which is the number this
/// whole change exists to remove.
#[test]
fn a_graphviz_row_is_written_centred_and_at_the_scale() {
    let fixture = bipartite();
    let row = ROWS
        .iter()
        .find(|row| row.name == "GRAPHVIZ_TWOPI")
        .expect("GRAPHVIZ_TWOPI is a row");
    let raw = run(row.motor.expect("twopi has a motor layout"), &fixture).expect("twopi");
    let post = run_row(row, &fixture).expect("twopi, post-convention");
    assert!(
        raw.iter().flatten().any(|v| v.abs() > SCALE),
        "the raw twopi output is already inside the unit box, so this says nothing"
    );
    for p in &post {
        assert!(
            p[0].abs() < SCALE && p[1].abs() < SCALE,
            "not centred and inside the unit box: {p:?}"
        );
    }
    let columns: [Vec<f64>; 2] = [
        post.iter().map(|p| p[0]).collect(),
        post.iter().map(|p| p[1]).collect(),
    ];
    let widest = columns.iter().map(|c| widest_of(c)).fold(0.0f64, f64::max);
    assert!(
        (widest - SCALE).abs() < 1e-12,
        "widest extent {widest} is not scale {SCALE}"
    );
}

/// `raw.max(axis=0) - raw.min(axis=0)` for one column, over a slice the caller owns.
fn widest_of(column: &[f64]) -> f64 {
    let lo = column.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = column.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    hi - lo
}

fn bipartite() -> super::super::super::fixtures::Fixture {
    super::super::super::fixtures::all()
        .expect("the fixture set")
        .into_iter()
        .find(|f| f.name == "bipartite")
        .expect("the bipartite fixture")
}

/// One coordinate per entry, as `struct.pack("<d", value).hex()`: the IEEE-754 double,
/// little-endian, which is the value itself.
///
/// **`float.hex()` was the first choice and is not one to paste.** It is
/// `PyOS_double_to_string`'s mode 3, which keeps trailing zeros and sometimes writes a
/// fourteenth fraction digit — numpy's `float.hex()` spells this same double both
/// `0x1.4ab138d71ff2p+42` and `-0x1.4ab138d71ff20p+42` for two different values whose shortest
/// forms differ by one character — so a typo in the padding cannot be seen and a value
/// cannot be round-tripped. The bit pattern can do both.
fn bits(points: &[[f64; 3]]) -> Vec<String> {
    points
        .iter()
        .flat_map(|p| p.iter())
        .map(|v| bits_of(v.to_bits()))
        .collect()
}

/// [`bits`] for one value: `to_le_bytes` is the same order `struct.pack("<d", …)` writes, so
/// no swap is needed and the hex reads the way the bytes do.
fn bits_of(raw: u64) -> String {
    raw.to_le_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

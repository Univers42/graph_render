//! The reference values, pasted from numpy. **Every constant below came out of
//! `ge-python-oracle` running numpy 2.3.3 on the four lines at `yifan_hu.py:318-325`, not out
//! of the code under test.** A constant copied from the function it is meant to check cannot
//! fail, and these are the constants that decide whether a Graphviz row is `bitwise` or
//! `shape`.

use super::super::super::{ROWS, SCALE};
use super::super::{run, run_row};
use super::{GRAPHVIZ_DIMS, left_to_right, scigraphs_graphviz_post};

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

/// The reduction order, pinned to numpy on the array shape SciGraphs actually reduces: an
/// `(n, 2)` **C-contiguous** array, `raw.mean(axis=0)` (`yifan_hu.py:318`).
///
/// **The pairwise control is the negative control, and it is what a wrong answer looks like.**
/// A 1-D probe cannot see this: numpy's `pairwise_sum_DOUBLE` only runs along the contiguous
/// axis, and on `raw.mean(axis=0)` of a C-contiguous array axis 0 is the strided one, so numpy
/// walks each column flat. [`pairwise_sum`] is that 1-D reduction, transcribed. Measured on this
/// probe, it differs from numpy on column 0 at **every** `n >= 8` and on column 1 at n = 16, 17,
/// 33, 127, 128, 129 and 300 — so the control is asserted on column 0, where it is
/// order-sensitive without exception, and on the agreement below eight, where its own branch
/// already *is* a left-to-right sum.
///
/// Both columns are pinned against numpy, so a reduction that read the wrong one is caught too.
#[test]
fn the_mean_of_an_n_by_2_array_is_the_left_to_right_sum() {
    const NUMPY: [(usize, &str, &str); 14] = [
        (1, "08003426f56b0c43", "fca9f1d24d62503f"),
        (2, "0c0062a25c94f942", "96438b6ce7fba93f"),
        (3, "5f554fb9143ef142", "3921d657a6c1b93f"),
        (5, "0f006b90a8abe442", "dd9bcc6590a8c93f"),
        (8, "80a7ffc485313ac2", "0cc00257f76bd63f"),
        (9, "2b723bce30b20442", "2fc3a8febf9ed93f"),
        (16, "18ff718d13ab94c2", "d6ec701ebb01e83f"),
        (17, "a6d1c30181285f42", "3c5fe0e8419be93f"),
        (33, "b24d163c7b14b942", "4f508d851b9af93f"),
        (64, "2380ce70d8d5a942", "643cdc0f5a330940"),
        (127, "061f8b4de2ae3042", "dff308663e331940"),
        (128, "4073ffc48531fac1", "050fef8671661940"),
        (129, "28ec3d63421ac741", "82bc15a8a4991940"),
        (300, "4444444444f08d3f", "a7a31a2569e62d40"),
    ];
    for (n, mean_x, mean_y) in NUMPY {
        let rows = probe(n);
        for (c, pinned) in [mean_x, mean_y].iter().enumerate() {
            let column: Vec<f64> = rows.iter().map(|r| r[c]).collect();
            let ours = left_to_right(&column) / n as f64;
            assert_eq!(bits_of(ours.to_bits()), *pinned, "n = {n}, column {c}: not numpy's mean");
            let control = pairwise_sum(&column) / n as f64;
            if c == 0 && n >= 8 {
                assert_ne!(control.to_bits(), ours.to_bits(), "n = {n}: the pairwise control agreed");
            } else if n < 8 {
                assert_eq!(control.to_bits(), ours.to_bits(), "n = {n}: below eight, one order only");
            }
        }
    }
}

/// The same seventeen-node `(17, 2)` array through all of `yifan_hu.py:318-325`: the mean, the
/// centring, the extent of the *centred* column and the `× scale`. The mean's own order is
/// [`the_mean_of_an_n_by_2_array_is_the_left_to_right_sum`]; this is the rest of the five lines,
/// on an array long enough for the order to matter in the written coordinates.
#[test]
fn seventeen_nodes_land_where_numpy_puts_them() {
    const NUMPY: [&str; 51] = [
        "596b196142fd0340", "51a96b7616ffe1bc", "0000000000000000", // row 0
        "3ba534f7ec15d0bf", "d10e6fc20483dfbc", "0000000000000000", // row 1
        "59454f26ca3a983f", "89147bc40404dbbc", "0000000000000000", // row 2
        "49b800e25c716fbf", "a22ca4d10e84d6bc", "0000000000000000", // row 3
        "62dfe8bf63d451bf", "b0e5d87cb603d2bc", "0000000000000000", // row 4
        "a794e69ebd0204c0", "6fde26ee5906cbbc", "0000000000000000", // row 5
        "62b5961126d4cf3f", "9a96a2aa0e05c2bc", "0000000000000000", // row 6
        "69eee30c69f89abf", "eeeb44884007b2bc", "0000000000000000", // row 7
        "0fdbb65acb08533f", "45d47bd387d310bc", "0000000000000000", // row 8
        "e0b861a98a055abf", "0fa3ac8ff7feb13c", "0000000000000000", // row 9
        "546b196142fd0340", "ffdf7eed1d01c23c", "0000000000000000", // row 10
        "4da534f7ec15d0bf", "9348c904c902cb3c", "0000000000000000", // row 11
        "9e444f26ca3a983f", "ea8eaa7e3d02d23c", "0000000000000000", // row 12
        "4ebc00e25c716fbf", "8276dc2e1983d63c", "0000000000000000", // row 13
        "63e5e8bf63d451bf", "52f59708f703db3c", "0000000000000000", // row 14
        "a894e69ebd0204c0", "f33e13a4d684df3c", "0000000000000000", // row 15
        "5bb5961126d4cf3f", "581ff8d8db02e23c", "0000000000000000", // row 16
    ];
    let rows = probe(17);
    let points: Vec<[f64; 3]> = rows.iter().map(|r| [r[0], r[1], 0.0]).collect();
    assert_eq!(bits(&scigraphs_graphviz_post(&points, GRAPHVIZ_DIMS, SCALE)), NUMPY);
}

/// Alternating signs against ten-to-the-fifteen magnitudes: cancellation heavy enough that
/// the order of the additions shows in the mean, and every term exactly representable so the
/// probe is the same array on both sides. The second column is a different probe, so that a
/// reduction which read the wrong one could not pass both halves of the table.
fn probe(n: usize) -> Vec<[f64; 2]> {
    const MAGNITUDE: [f64; 5] = [1e15, 1e14, 1e13, 1e12, 1e11];
    const SIGN: [f64; 2] = [1.0, -1.0];
    (0..n)
        .map(|i| {
            [
                SIGN[i % 2] * MAGNITUDE[i % 5] + 1.0 / (i + 1) as f64,
                i as f64 * 0.1 + 1e-3 / (i + 1) as f64,
            ]
        })
        .collect()
}

/// numpy's `pairwise_sum_DOUBLE`, transcribed: the reduction it runs along the **contiguous**
/// axis, and the negative control for [`the_mean_of_an_n_by_2_array_is_the_left_to_right_sum`].
/// It is here and not in `gv_post.rs` because no reference path runs it.
fn pairwise_sum(values: &[f64]) -> f64 {
    const PW_BLOCKSIZE: usize = 128;
    let n = values.len();
    if n < 8 {
        return left_to_right(values);
    }
    if n > PW_BLOCKSIZE {
        let split = (n / 2) & !7;
        return pairwise_sum(&values[..split]) + pairwise_sum(&values[split..]);
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

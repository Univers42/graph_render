//! The seventeen-node round trip: all of `yifan_hu.py:318-325` on one array, with numpy's own
//! answer pasted in. It lives beside `super`, not inside it, because `super` was past the
//! 300-line file limit once this constant landed.

use super::{GRAPHVIZ_DIMS, SCALE, bits, probe, scigraphs_graphviz_post};

/// numpy's 51 written coordinates for a 17-node `(17, 2)` array, as
/// `struct.pack("<d", v).hex()` — the IEEE-754 double, little-endian, three per row with the
/// z slot the `np.zeros` at `:323` wrote in the middle.
const NUMPY: [&str; 51] = [
    "596b196142fd0340",
    "51a96b7616ffe1bc",
    "0000000000000000", // row 0
    "3ba534f7ec15d0bf",
    "d10e6fc20483dfbc",
    "0000000000000000", // row 1
    "59454f26ca3a983f",
    "89147bc40404dbbc",
    "0000000000000000", // row 2
    "49b800e25c716fbf",
    "a22ca4d10e84d6bc",
    "0000000000000000", // row 3
    "62dfe8bf63d451bf",
    "b0e5d87cb603d2bc",
    "0000000000000000", // row 4
    "a794e69ebd0204c0",
    "6fde26ee5906cbbc",
    "0000000000000000", // row 5
    "62b5961126d4cf3f",
    "9a96a2aa0e05c2bc",
    "0000000000000000", // row 6
    "69eee30c69f89abf",
    "eeeb44884007b2bc",
    "0000000000000000", // row 7
    "0fdbb65acb08533f",
    "45d47bd387d310bc",
    "0000000000000000", // row 8
    "e0b861a98a055abf",
    "0fa3ac8ff7feb13c",
    "0000000000000000", // row 9
    "546b196142fd0340",
    "ffdf7eed1d01c23c",
    "0000000000000000", // row 10
    "4da534f7ec15d0bf",
    "9348c904c902cb3c",
    "0000000000000000", // row 11
    "9e444f26ca3a983f",
    "ea8eaa7e3d02d23c",
    "0000000000000000", // row 12
    "4ebc00e25c716fbf",
    "8276dc2e1983d63c",
    "0000000000000000", // row 13
    "63e5e8bf63d451bf",
    "52f59708f703db3c",
    "0000000000000000", // row 14
    "a894e69ebd0204c0",
    "f33e13a4d684df3c",
    "0000000000000000", // row 15
    "5bb5961126d4cf3f",
    "581ff8d8db02e23c",
    "0000000000000000", // row 16
];

/// The same seventeen-node `(17, 2)` array through all of `yifan_hu.py:318-325`: the mean, the
/// centring, the extent of the *centred* column and the `× scale`. The mean's own order is
/// [`super::the_mean_of_an_n_by_2_array_is_the_left_to_right_sum`]; this is the rest of the five
/// lines, on an array long enough for the order to matter in the written coordinates.
#[test]
fn seventeen_nodes_land_where_numpy_puts_them() {
    let rows = probe(17);
    let points: Vec<[f64; 3]> = rows.iter().map(|r| [r[0], r[1], 0.0]).collect();
    assert_eq!(
        bits(&scigraphs_graphviz_post(&points, GRAPHVIZ_DIMS, SCALE)),
        NUMPY
    );
}

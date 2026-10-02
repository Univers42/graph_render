//! Rows 22 to 30: `YIFAN_HU` and the eight `GRAPHVIZ_` names — every row whose reference
//! arm is the engine rather than SciGraphs' own Python, and the only family that carries
//! a `reference_note`.
//!
//! **A move, not a change.** Every value below is the byte-for-byte content of the one
//! table `table.rs` held before it was split along its row families; a re-pinned row is
//! still edited here and nowhere else.

use super::super::{Baseline, row};

pub(super) const GRAPHVIZ: [Baseline; 9] = [
    row(
        "YIFAN_HU",
        "3323aa8cb20ceb50e5b3ae7afbb9d69d0afd4acd20c05f561d0e6bb72dd2f99f",
        "4c90e6c3493b207255f3be114f517f41ceb234b7ff75e241a85991b400b9144f",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_DOT",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "4b608db68705ee41ad12270feff2d1e17c0af7777aae95c11f583c69f37e6f93",
        "",
        f64::INFINITY,
        "shape",
        "reference-absent",
    ), // not run: not run: no motor layout for this name
    row(
        "GRAPHVIZ_NEATO",
        "9f3fdfef4f5bcba3af60711100fb885550ce7a9b5c15c957cc68871ab7f1e8cf",
        "d866a341ba931c2a5b0be5f0c755960ed6d26c08f7e672b867e1396533909535",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "GRAPHVIZ_FDP",
        "76dde17b308546a000e08754ee3345bac6fd6bc336e38761a98a52b598dab3b7",
        "",
        "the engine's own start is not seeded by -Gstart: two runs differ",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "GRAPHVIZ_SFDP",
        "61d65b0768faf4a9118540c690cbb8bcd4ffc57da782b2cdf27f7b4940805f7b",
        "4c90e6c3493b207255f3be114f517f41ceb234b7ff75e241a85991b400b9144f",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_TWOPI",
        "4dc1e046a8d63af0eb7ad1196b5fbbafd1d82883f47c7a3a6d5f2893902cd9b4",
        "b757bd447593d9bd6af8dbb09d7e7065f9683c98d47ca6a28e5a82fe0e03cbbe",
        "",
        1e-9,
        "bitwise",
        "convention",
    ),
    row(
        "GRAPHVIZ_CIRCO",
        "7272c7df98b0da15b39b3b638ea85ca9cf83c16f394ffa6bde924e04c554c400",
        "43bd5ccddf7176066c95e8dc000fdb8ca20396fce5427d762bbf4c3d4f4b7907",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_OSAGE",
        "dcf04e18cf2d24e42c141bda41a8498cc00bebafdc76688ae0bf5fb2686d5da5",
        "5dcfc6baa37356af16bc4dcb1390d9bca43b32cc0824b7fe9664e787391fb2b9",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_PATCHWORK",
        "db3d36c1b1cbdb589593120d908b508d7b86ebc1b36e0b11a6f522aca321ec08",
        "cdda1c9c29be3f24cde803e1e51232cfab42d8304c444357c36a79a9f7ee4e3e",
        "",
        1e-9,
        "bitwise",
        "convention",
    ),
];

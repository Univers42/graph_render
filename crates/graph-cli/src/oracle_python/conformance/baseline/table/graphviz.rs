//! Rows 22 to 30: `YIFAN_HU` and the eight `GRAPHVIZ_` names — every row whose reference
//! arm is the engine rather than SciGraphs' own Python, and the only family that carries
//! a `reference_note`.
//!
//! **A move, not a change.** Every value below is the byte-for-byte content of the one
//! table `table.rs` held before it was split along its row families; a re-pinned row is
//! still edited here and nowhere else.
//!
//! **These nine were re-pinned once, together, by `sg-graphviz-scale`,** when SciGraphs'
//! own five lines over the engine's points (`yifan_hu.py:318-325`) moved to both arms
//! (`motor/gv_post.rs`, `sc_graphviz.py`). Only the bytes moved: every tier and every cause
//! below is the one `sc_propose.classify` had already decided, and the two `convention` rows'
//! Procrustes disparity is unchanged to the last bit a `f64` carries.

use super::super::{Baseline, row};

pub(super) const GRAPHVIZ: [Baseline; 9] = [
    row(
        "YIFAN_HU",
        "de201a3bf0493b578133c2d1c1dbaf65a19fe3c62147b3db7e846701e72a9a9c",
        "acfb41636d08eae9d9c0779d7b2191411df36442ef14fee2b0ce40ea7da14b92",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_DOT",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "0d71b1766e1cbad6bbd1766876acf5b3f0a9c77d1a2040e87ea5fa9daac1fa70",
        "",
        f64::INFINITY,
        "shape",
        "reference-absent",
    ), // not run: not run: no motor layout for this name
    row(
        "GRAPHVIZ_NEATO",
        "7d10d2294436aa58fc47c8adaaf0c135624abe30f802309e94acf1d79c986f20",
        "bb56c471f0bb738ac053db47cc6fbf4083c09732c405a088d07c032c4c8ad61a",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "GRAPHVIZ_FDP",
        "f02167cf01b4f64145db4779004eb397b36ff482fb37008be898cca971b20f6f",
        "",
        "the engine's own start is not seeded by -Gstart: two runs differ",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "GRAPHVIZ_SFDP",
        "7b7aeabb59599b55eee5ed41b82ee8ce5431c396e3ad7079be1c4d8e7fb2bd6f",
        "acfb41636d08eae9d9c0779d7b2191411df36442ef14fee2b0ce40ea7da14b92",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_TWOPI",
        "2d2e4b04fa8850aa89574d6d635ec0de43409a26039cb28131d71584e6247bb9",
        "2e1368a0c2bbf13231ea26434ce6b8db691272d725604c24f53bd766e55d8eb5",
        "",
        1e-9,
        "bitwise",
        "convention",
    ),
    row(
        "GRAPHVIZ_CIRCO",
        "1ca5ecf45c65d2488dae1670de6275e9334998271b654c29530192caa627c4ea",
        "f771a0a4e2498c868790052fb0f43c5aa48d7f140aa918adf504f504d914b205",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_OSAGE",
        "f27bfe606672fb81b7ffb0f342693f5c2fb6154a2a1571f3a8e9ed9683fa4e15",
        "c8507df100b08db5472201e5f34ec11a546966e8f5ef525257cb47d392e8916e",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_PATCHWORK",
        "969e5e8fe3524597245ab459ec6afecdc0847a40225730b8c6a2b0c6733bfba5",
        "3af27cc569902b957988b1d64284351117614d53c1e9558774d7fc7fd3300a79",
        "",
        1e-9,
        "bitwise",
        "convention",
    ),
];

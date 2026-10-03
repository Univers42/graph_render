//! Rows 22 to 30: `YIFAN_HU` and the eight `GRAPHVIZ_` names — every row whose reference
//! arm is the engine rather than SciGraphs' own Python, and the only family that carries
//! a `reference_note`.
//!
//! **A move, not a change.** Every value below is the byte-for-byte content of the one
//! table `table.rs` held before it was split along its row families; a re-pinned row is
//! still edited here and nowhere else.
//!
//! **These nine were re-pinned twice, together, by `sg-graphviz-scale`.** First when
//! SciGraphs' own five lines over the engine's points (`yifan_hu.py:318-325`) moved to both
//! arms (`motor/gv_post.rs`, `sc_graphviz.py`); only bytes moved then. Second when the
//! reference arm stopped reading the engine's `-Tplain` **text** and read `ND_coord` itself
//! through `gv_exact.c`, because that text is `%.5g` (`lib/common/output.c:66-71`) and its
//! step floored `GRAPHVIZ_TWOPI`'s `max_gap` at 7.5e-5. This time **two tiers moved**, and
//! they are the point of the change:
//!
//! | row | `max_gap` before | `max_gap` after | tier before | tier after |
//! |---|--:|--:|---|---|
//! | `GRAPHVIZ_TWOPI` | 7.46e-05 | 1.70e-07 | `bitwise`/`convention` | `tolerance`/`arithmetic` |
//! | `GRAPHVIZ_PATCHWORK` | 2.37e-04 | 1.81e-07 | `bitwise`/`convention` | `tolerance`/`arithmetic` |
//!
//! The other seven rows kept their tier and their cause: their `max gap` is a different
//! picture, hundreds of points apart, and no reference precision reaches that.

use super::super::{Baseline, row};

pub(super) const GRAPHVIZ: [Baseline; 9] = [
    row(
        "YIFAN_HU",
        "de201a3bf0493b578133c2d1c1dbaf65a19fe3c62147b3db7e846701e72a9a9c",
        "c6f3141123dedb84bfa577fb2b1b3d971a05cd3fee46bc5b653f42ef46479aab",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_DOT",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        "e5c590f15690d98b29ae410c3a8d9db361dba8feb967f0a031a06726b9738697",
        "",
        f64::INFINITY,
        "shape",
        "reference-absent",
    ), // not run: not run: no motor layout for this name
    row(
        "GRAPHVIZ_NEATO",
        "7d10d2294436aa58fc47c8adaaf0c135624abe30f802309e94acf1d79c986f20",
        "a36d5bb664d5531725b43d1ed882b5d862c02c6536c51d213df3ccd67e335c44",
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
        "c6f3141123dedb84bfa577fb2b1b3d971a05cd3fee46bc5b653f42ef46479aab",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_TWOPI",
        "9ed0d1c553708adc59f0fc6e1cc0e6b0ed9aa25cf10583a28e58f3eb93935d02",
        "33b9a17dca39107ceea70792c2b1fc7c2c7a32e28175b939482878dd1b3fa40b",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
    row(
        "GRAPHVIZ_CIRCO",
        "1ca5ecf45c65d2488dae1670de6275e9334998271b654c29530192caa627c4ea",
        "44c7d9126464d770bdb899791363f3295a57acda8e6e9fa0079d040cb87cb886",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_OSAGE",
        "f27bfe606672fb81b7ffb0f342693f5c2fb6154a2a1571f3a8e9ed9683fa4e15",
        "935bacd6cc4dcf63fd9b29524f98395436958f40b18c46f316a2592c76facd94",
        "",
        1e0,
        "shape",
        "algorithm",
    ),
    row(
        "GRAPHVIZ_PATCHWORK",
        "365bff26245b3ebe3e3aba5d3050e7ad0e41f58164a4916b4510b3c256505a21",
        "ccad3e6819853a883c9c8afc1638682059c1ed5bd9b552a7157fd1a28b183b35",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
];

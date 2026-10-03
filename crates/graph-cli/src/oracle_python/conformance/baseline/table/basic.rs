//! The first six rows of the table: `RANDOM`, `GRID`, `SPRING`, `SPRING_3D`,
//! `CIRCLE_PACKING` and `FORCEATLAS2` — the layouts SciGraphs runs its own code for.
//!
//! **A move, not a change.** Every value below is the byte-for-byte content of the one
//! table `table.rs` held before it was split along its row families; a re-pinned row is
//! still edited here and nowhere else.

use super::super::{Baseline, row};

pub(super) const BASIC: [Baseline; 6] = [
    row(
        "RANDOM",
        "79ac64fb162056eeed4ed03ed2d91f6d9463ac88d87594870b21a075e713f21e",
        "97db3956653c5a3e688903ee93ec114a69e2a063ecc9d8b1595b243c5d5555f9",
        "",
        1e-14,
        "tolerance",
        "arithmetic",
    ),
    row(
        "GRID",
        "d0f4b0a4db20601c26fa313b58f048b33519901ffd9a952047b8d5079a53bcd2",
        "bdf2552edff2098c56eda31608335447ffb6be13663bcf241de8b556b2f5522e",
        "",
        1e-31,
        "tolerance",
        "arithmetic",
    ),
    row(
        "SPRING",
        "10a4ce55bde3fde19b8e01d988ba9191842e1a3cc74f17bbeb9bc10e78b4d4d9",
        "22f7f5e01bb3ee6ce16a73796cce5d2f3a66d3bb82a3a710461a95d91915a7ec",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
    row(
        "SPRING_3D",
        "eadfb1c967846e50bebe06c66765bb39f28c2b2f566161d86f52b185b86f1acc",
        "0a6e3c9035a5342d6d960d99b96b4b651f35942bb2db95eede6e7c88abea9515",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
    row(
        "CIRCLE_PACKING",
        "152f3a67739b2ff42775691137eb2c100c50e168e99b17fbdc08d07884e843b2",
        "4b1010b0b6d407add18f9b3f8852eaf1912229567cd02f477ca33dee299cbdba",
        "",
        1e-15,
        "shape",
        "algorithm",
    ),
    row(
        "FORCEATLAS2",
        "82ce9bfb8d69cb73399638e5e9d88477b0a37d39796e02702718cf4a1ec3bc37",
        "adc82f0e73c0abcf66242d5ebc452967404aff4d8b1392119a99aa140b34b2cb",
        "",
        1e-12,
        "bitwise",
        "convention",
    ),
];

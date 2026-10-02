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
        "7ef3a5f7a9f8c45a8d1f2062bf059b1d96b6ec173e6b358f177556bf8262c8e3",
        "97db3956653c5a3e688903ee93ec114a69e2a063ecc9d8b1595b243c5d5555f9",
        "",
        1e0,
        "bitwise",
        "rng",
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
        "e8e0ce6226082434791e6e7debd892b5262168c1336b3feab7574b09d1292bd1",
        "22f7f5e01bb3ee6ce16a73796cce5d2f3a66d3bb82a3a710461a95d91915a7ec",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "SPRING_3D",
        "7cfcdd38ce96eb9113a08ffda8a43a55f79f5dbf5f0addf40049b735139f097f",
        "0a6e3c9035a5342d6d960d99b96b4b651f35942bb2db95eede6e7c88abea9515",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "CIRCLE_PACKING",
        "c143bfd75b5c1f85c015ead0154ec955d4a7684a8f4b27cb33db4cf0051a8d5b",
        "4b1010b0b6d407add18f9b3f8852eaf1912229567cd02f477ca33dee299cbdba",
        "",
        1e-15,
        "shape",
        "algorithm",
    ),
    row(
        "FORCEATLAS2",
        "e58ef84a2bf7c7393d91ada550f095a5c476189a92554db087bdf9c0f1f42b49",
        "adc82f0e73c0abcf66242d5ebc452967404aff4d8b1392119a99aa140b34b2cb",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
];

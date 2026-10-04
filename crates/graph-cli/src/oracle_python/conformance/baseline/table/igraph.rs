//! Rows 19 to 21: `IGRAPH_DH`, `IGRAPH_GRAPHOPT` and `MDS_3D`, the three rows whose layout
//! is a projection rather than a placement.
//!
//! **A move, not a change.** Every value below is the byte-for-byte content of the one
//! table `table.rs` held before it was split along its row families; a re-pinned row is
//! still edited here and nowhere else.

use super::super::{Baseline, row};

pub(super) const IGRAPH: [Baseline; 3] = [
    row(
        "IGRAPH_DH",
        "9653d16dbe0747a61096bb8b973958da5b9ecf9983e43684f679ed6e13113c24",
        "eead3cc757753c0088305f9560dcab4650eda1e96ab3a51f20557a0a6772ad71",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "IGRAPH_GRAPHOPT",
        "d2dfe0552e463f98295b7dbbb684f1dc4f33b74180fd77d2359169b327c2571a",
        "407d050b71a481b869f8c5238cb63a2e620858620e9f2abd1bc3d0164afdf2a2",
        "",
        1e0,
        "bitwise",
        "rng",
    ),
    row(
        "MDS_3D",
        "49e3d8aaa70c8273d1205dc682fc62f0370418ad2760b2d8ce127fded381aa09",
        "603f394d3446e83ad9e0dd88ba272b9feb6a2fc8107404bc63e4a06ca29ebc0f",
        "",
        1e-15,
        "bitwise",
        "convention",
    ),
];

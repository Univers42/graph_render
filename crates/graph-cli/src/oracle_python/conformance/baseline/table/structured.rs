//! The last two rows: `SUGIYAMA` and `CIRCULAR_HIERARCHY`, the rows a drawing derives from a
//! structure rather than from a set of forces.
//!
//! **A move, not a change.** Every value below is the byte-for-byte content of the one
//! table `table.rs` held before it was split along its row families; a re-pinned row is
//! still edited here and nowhere else.

use super::super::{Baseline, row};

pub(super) const STRUCTURED: [Baseline; 2] = [
    row(
        "SUGIYAMA",
        "fb980d82ceb1ff44b0707ba5c38598d7b3e4e7b31b07c71b3604fbf82c8240cd",
        "308abc273e6048eb3a91c3bf70cbff7c932dd5ddc0186d62c49f793dcce56b1d",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
    row(
        "CIRCULAR_HIERARCHY",
        "ab6094a3f836f5e8e581554c9507cdb59b97e146a4a3d91d63efcd01c4ae9596",
        "8d51906c2ae449a59860a9369ea006f2ac63c3fee8dc247a9699dbbaa714fe54",
        "",
        1e-15,
        "tolerance",
        "arithmetic",
    ),
];

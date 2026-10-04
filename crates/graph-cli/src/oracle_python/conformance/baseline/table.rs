//! The measured matrix, in `super::ROWS` order: 32 lines of seven pinned values each.
//!
//! **One module per row family, and this file is the index that reads them back in matrix
//! order.** `basic.rs` holds rows 1-6, `networkx.rs` 7-18, `igraph.rs` 19-21, `graphviz.rs`
//! 22-30 and `structured.rs` the last two. A repair job re-pins its row in the family file
//! that owns it; nothing here states a sha256, so a diff of a repair names the row and the
//! family in the same breath.
//!
//! The families are contiguous blocks of `super::ROWS` rather than free groups, so the
//! table is still read top to bottom in matrix order — [`BASELINE`] is that order, and the
//! per-family sizes are in the arrays' own types.
//!
//! Regenerate with `harness/scigraphs-conformance.py --metrics`, which writes
//! `target/scigraphs-conformance/conformance-baseline-proposed.rs` in this syntax: a run's
//! numbers pasted in without being retyped, because a retyped sha256 is a typo nobody reads.

mod basic;
mod graphviz;
mod igraph;
mod networkx;
mod structured;

use basic::BASIC;
use graphviz::GRAPHVIZ;
use igraph::IGRAPH;
use networkx::NETWORKX;
use structured::STRUCTURED;

/// Each family's own length, read off its array: a family that gained or lost a row is a
/// compile error in [`table`] rather than a shifted row at the gate.
const N_BASIC: usize = BASIC.len();
const N_NETWORKX: usize = NETWORKX.len();
const N_IGRAPH: usize = IGRAPH.len();
const N_GRAPHVIZ: usize = GRAPHVIZ.len();

/// The pinned matrix, the five families laid end to end in `super::ROWS` order.
///
/// **Copied out of the families one row at a time, in a `const fn`,** because
/// `std::array::from_fn` is not callable in a const item on stable and a `LazyLock` would
/// stop `BASELINE` being a `const` the other module's `const _:` assertion reads. The
/// literal `32` is the matrix's own row count, so a family that no longer fills its block
/// indexes out of range here and the table stops compiling rather than shifting a row.
const fn table() -> [super::Baseline; 32] {
    // Seeded from the first row and overwritten slot by slot: every index below is inside
    // the family it reads, so no slot survives the loop.
    let mut out = [BASIC[0]; 32];
    let mut i = 0;
    while i < N_BASIC {
        out[i] = BASIC[i];
        i += 1;
    }
    while i < N_BASIC + N_NETWORKX {
        out[i] = NETWORKX[i - N_BASIC];
        i += 1;
    }
    while i < N_BASIC + N_NETWORKX + N_IGRAPH {
        out[i] = IGRAPH[i - N_BASIC - N_NETWORKX];
        i += 1;
    }
    while i < N_BASIC + N_NETWORKX + N_IGRAPH + N_GRAPHVIZ {
        out[i] = GRAPHVIZ[i - N_BASIC - N_NETWORKX - N_IGRAPH];
        i += 1;
    }
    while i < 32 {
        out[i] = STRUCTURED[i - N_BASIC - N_NETWORKX - N_IGRAPH - N_GRAPHVIZ];
        i += 1;
    }
    out
}

/// The pinned matrix, the five families read back in `super::ROWS` order.
pub const BASELINE: &[super::Baseline] = &table();

//! The ANALYSIS stage over the ABI (`docs/contract/wasm-abi.md` "ANALYSIS"): which
//! analysis `gm_analysis_run`'s index names, and the framed canonical-JSON face each one
//! returns.
//!
//! **Every row is graph-core's own function, called as it is.** Nothing in
//! `graph_core::analysis` is re-derived, re-weighted or re-ordered here: this module is
//! the registry (`id` + a `fn(&Topology)`), the JSON writer, and, for depth only, the one
//! place `Hierarchy::of`'s refusal becomes a caller-bug panic — `depth::bfs_depth` then
//! takes `&Hierarchy` directly, because graph-core implements `Roots` on that type. A number
//! that differs between this face and the one graph-core's own tests pin is a bug in one of
//! the two, and the tests below compare them rather than restating expectations.
//!
//! **What the JSON says, and what it does not.** Keys are in ascending order, one fixed
//! order for every analysis, so the face is canonical and byte-comparable (D7: the text
//! is the value). Every result carries `id`, `kind` (`"f64"` or `"u32"` — the element
//! type of `values`), `nodeCount` and `values` in dense-index order. Three analyses add
//! the one scalar graph-core hands back beside the column, because dropping it would
//! drop the escape hatch its own `Ponytail` marker names: `converged` for the
//! eigenvector iteration, `modularity` for Louvain, `max` for depth.
//!
//! **A path query is not here.** `graph_core::analysis::paths` needs a source node and a
//! mode, neither of which the `(handle, index)` signature can carry without inventing a
//! convention; it stays a graph-core-only capability until the ABI has a way to ask.
//!
//! Determinism: no clock, no RNG, no `HashMap` iteration, and a `Vec` in dense-index
//! order out of every function called here (D2, D3, D4, D5, D8, D10).
//!
//! # The shape of the tree
//!
//! One concern per child, so no file here has to grow past the house's 300-line limit and
//! so a reader looking for one thing has one place to look:
//! [`report`] holds the two types the face is written from and the writer itself,
//! [`registry`] holds the table and the four functions the export delegates to, and
//! `components`/`communities`/`centrality`/`depth` each hold one family of adapters — one
//! per row, so a row and the graph-core function it calls cannot be confused.
//!
//! **This facade is also the hash gate's byte producer.** graph-cli's native arm hashes
//! [`to_json`] over the same topology the wasm arm's `gm_analysis_run` reads, so the two
//! arms compare one implementation's bytes rather than two implementations' agreement —
//! which is the only way that comparison can say anything about wasm32 at all.

mod centrality;
mod communities;
mod components;
mod depth;
mod registry;
mod report;

#[cfg(test)]
mod tests;

pub use registry::{ANALYSES, count, id_at, run, to_json};
pub use report::{Column, Entry, Report};

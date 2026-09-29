//! Ingest: the neutral contract, in three parts.
//!
//! - [`strength`] — the one edge-strength table by edge kind. This slice.
//! - `roles` — declared roles, not type sniffing. Not yet built.
//! - `build` — ingest → topology, the single derivation. Not yet built.
//!
//! Graph derivation used to exist in three places with three constant tables that had
//! already diverged, so two live code paths produced different layouts for the same
//! data. [`strength`] is the fix: one table, one citation, one set of snapshot hashes.

pub mod strength;

pub use strength::{STRENGTH_TABLE, edge_strength};

//! Ingest: the neutral contract, in three parts.
//!
//! - [`roles`] — the eight declared roles, and how a document is read through them.
//! - [`build`] — ingest → nodes and edges. **One** derivation, for every source.
//! - [`strength`] — the one edge-strength table by edge kind.
//!
//! ## Why the contract exists at all
//!
//! The host reverse-engineered semantic roles from its own property-type strings — "the
//! first `multi_select`, or a field named `/^tags?$/i`"; "the first `status`, else the
//! first `select`". That is a 20-member vendor type enum standing in for eight roles,
//! and it is why the engine was Notion-shaped rather than data-source-shaped. It also
//! produced **H6**: `groupValue` depended on `Object.values()` ordering, which is not
//! stable across a JSON reserialization, so the same data could draw differently
//! depending on how it had been written out and read back.
//!
//! A **declared** role has neither problem. The declaration is what is read, so a
//! document's meaning does not depend on its field names or its member order, and the
//! derivation in [`build`] is written once for every source instead of once per vendor.
//!
//! Graph derivation used to exist in three places with three constant tables that had
//! already diverged, so two live code paths produced different layouts for the same
//! data. [`strength`] is the table that replaced them, and [`build`] is the single
//! reader of it — this module is where the host's three copies become one.

pub mod build;
pub mod roles;
pub mod strength;

#[cfg(test)]
mod tests;

pub use build::{BuildError, Derived, build, build_topology, describe, to_canonical_json};
pub use roles::DEFAULT_WEIGHT;
pub use strength::{STRENGTH_TABLE, edge_strength};

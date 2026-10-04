//! The POST stage over the ABI (`docs/contract/wasm-abi.md` "POST"): which edge-geometry
//! capability `gm_post_run`'s index names, and the adapters that fit graph-core's own
//! entry points into the one `PostRun` signature.
//!
//! **The table is a table, not a slice of `graph_core::post::POSTS`.** graph-core's own
//! registry holds the two bundlers; routing ([`graph_core::post::routed`]) and the four
//! styles ([`graph_core::post::styles`]) have their own entry points that take their
//! parameters explicitly, so a row here is a *thin adapter* over one of them rather than a
//! change to a signature graph-core already publishes. Nothing in graph-core is edited for
//! this, and a capability registered there later is one row away.
//!
//! **Every row runs at its own pinned defaults** — `GridParams::default()` for the route,
//! `StyleParams::for_style` for a style — because a hashed result is pinned to a default
//! and a caller reads the id, not a parameter block (the same rule `gm_run` follows, C2).
//!
//! Determinism: the table is a `static` in one fixed order, and every row delegates to
//! graph-core's own deterministic pass. `bundled` is read back only through
//! `graph_core::layout::snapshot`, so nothing here re-orders a node or an edge.
//!
//! # The shape of the tree
//!
//! One concern per child, for the house's 300-line limit and for the same reason as
//! [`crate::analysis`]: `registry` holds the table and the functions `gm_post_run`
//! delegates to, `bundlers`/`routed`/`styles` hold one family of adapters each, and
//! [`snapshot`] is the one byte path both the wasm export and graph-cli's native arm read.
//!
//! **That last one is the point of the split.** A hashed POST stage must be the *same*
//! bytes on both arms or the differential measures two implementations instead of two
//! targets, so the snapshot a pass produces is derived once, here, and both arms read it.

mod bundlers;
mod registry;
mod routed;
mod styles;

#[cfg(test)]
mod tests;

pub use registry::{CAPABILITIES, Entry, ROUTE_ID, count, id_at, run, snapshot};

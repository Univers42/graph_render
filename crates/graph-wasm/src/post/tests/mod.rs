//! The ABI's POST registry, its adapters, and every branch they can take. Natively
//! testable because `super` is: the table is plain data and every entry point is a
//! `PostRun` over graph-core's own types, so the wasm32-only export is a three-line
//! delegation (C21) and what it delegates to is pinned here.
//!
//! One child per claim, for the house's 300-line limit: [`fixtures`] holds the topology
//! and geometry builders every child shares, [`composability`] the claim that a
//! capability works over whatever a layout emitted, [`rows`] the per-row behaviour —
//! edges replaced, nodes and notes alone, the CSR well formed, the bundlers graph-core's
//! own entry points — [`styles`] the four style variants, and [`registry`] the table's
//! own structure. Each child names its own imports rather than inheriting this module's.

mod composability;
mod fixtures;
mod registry;
mod rows;
mod styles;

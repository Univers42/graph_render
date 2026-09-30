//! The ABI's ANALYSIS registry and its JSON face, over graph-core's own functions.
//! Natively testable because `super` is: the table is plain data, every entry point is a
//! pure function over a [`Topology`], and the JSON is written here rather than through a
//! wasm pointer.
//!
//! One child per claim, so no file grows past the house's 300-line limit: [`fixtures`]
//! holds the topology builders every child shares, [`column`] and [`columns`] the column
//! and registry-value pins, [`json`] the exact JSON face, [`registry`] the table's own
//! structure, and [`rows`] the claim that each row calls the graph-core function its id
//! names. Each child names its own imports rather than inheriting this module's.

mod column;
mod columns;
mod fixtures;
mod json;
mod registry;
mod rows;

//! The ABI's ANALYSIS registry and its JSON face, over graph-core's own functions.
//! Natively testable because `super` is: the table is plain data, every entry point is a
//! pure function over a [`Topology`], and the JSON is written here rather than through a
//! wasm pointer.

mod column;
mod fixtures;
mod json;
mod registry;
mod rows;
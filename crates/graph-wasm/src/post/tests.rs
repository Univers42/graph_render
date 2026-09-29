//! The ABI's POST registry, its adapters, and every branch they can take. Natively
//! testable because `super` is: the table is plain data and every entry point is a
//! `PostRun` over graph-core's own types, so the wasm32-only export is a three-line
//! delegation (C21) and what it delegates to is pinned here.

mod composability;
mod fixtures;
mod registry;
mod rows;
mod styles;
//! The declared roles, the one derivation, the id grammar and the convergence pair.
//!
//! Split into four modules under this directory for the house's 300-line limit;
//! `support.rs` holds the one fixture they all build on, so a test states the
//! document it means instead of quoting JSON fourteen times.

mod build;
mod convergence;
mod edges;
mod grammar;
mod roles;
mod support;

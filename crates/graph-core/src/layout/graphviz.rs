//! The native ports of Graphviz's own engines, one module per engine.
//!
//! `circo` is here first. Every engine in this tree is compared against the docker-only
//! Graphviz oracle rather than against a second run of ours
//! (`docs/decisions/graphviz-oracle.md`), and every one of them is a port read from the
//! pinned Graphviz release as an algorithm reference — never a translation, never a link.

pub mod circo;

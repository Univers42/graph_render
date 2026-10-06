//! The read half of §5.2's table: `/graph`, `/changes`, `/v1/meta` and `/v1/workspaces`.
//!
//! Three files, one per route family, and [`common`] for the three fixtures they share. The two
//! that matter for Review Focus are in [`graph`]: `/layout`'s byte equality is Task 8's, and
//! `/graph` is what the relay streams, so `the_same_cursor_gives_the_same_bytes` and
//! `graph_never_buffers_a_whole_document` are the properties the relay rests on.
#![cfg(feature = "db-tests")]

#[path = "read/changes.rs"]
mod changes;
#[path = "read/common.rs"]
mod common;
#[path = "read/graph.rs"]
mod graph;
#[path = "read/meta.rs"]
mod meta;
mod support;

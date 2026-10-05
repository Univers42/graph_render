//! §5.3's notice stream: `GET /v1/workspaces/{ws}/events`.
//!
//! Two files. [`stream`] covers the happy path and the resume rule; [`ends`] covers the two early
//! closes, which is where the plan's Review Focus 3 lives — `busy_carries_no_id_line`,
//! `the_busy_slot_is_free_before_the_close`, `the_busy_reconnect_reads_no_graph` and
//! `a_cursor_pruned_during_the_backoff_gets_resync` are all in `ends`, by those names.
#![cfg(feature = "db-tests")]

#[path = "events/common.rs"]
mod common;
#[path = "events/ends.rs"]
mod ends;
#[path = "events/stream.rs"]
mod stream;
mod support;

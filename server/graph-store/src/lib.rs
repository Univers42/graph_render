//! graph-hub's PostgreSQL store: the schema, the epoch clock and its triggers, the restore
//! detector, the writer transaction, the streamed materializer, `/changes` after a cursor, a
//! plugin's own records, retention and the idempotency sweeper.
//!
//! This crate exports types and functions. It never names an HTTP status, opens a listener or
//! reads a key file; those are the hub's own business (slice 3).
//!
//! Every write path opens its transaction with `set_config('hub.writer', '1', true)`, so the
//! `ENABLE ALWAYS` triggers on every workspace-keyed table stay quiet for the hub's own writes
//! and fire for everybody else's.
#![forbid(unsafe_code)]

pub mod breaks;
pub mod config;
pub mod epoch;
pub mod error;
pub mod hooks;
pub mod migrate;
pub mod pool;
pub mod store;
pub mod writer;

pub use config::{StoreConfig, check};
pub use error::{DbError, StoreError};
pub use pool::Detector;
pub use store::Store;
pub use writer::{BatchOutcome, BatchWrite, Idempotency, ManifestWrite, ManifestWritten};

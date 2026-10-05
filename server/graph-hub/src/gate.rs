//! The four semaphores, created once at start from §6's `Gates`.
//!
//! The hub's queue is unbounded and its wait is a **timeout**, not a queue-full check, which is the
//! one thing that differs from graph-server's own gate (`server/graph-server/src/gate.rs:31-44`,
//! where a full queue is a 429 at once). So [`Gate::admit`] has no queue counter and no 429 at all:
//! [`crate::gate::subscribers::Subscribers`] is the only 429 in the crate, and it counts streams
//! rather than requests.
//!
//! Each route takes exactly one permit, and `GRAPH_HUB_TIMEOUT_MS` bounds the **wait** and never the
//! work, so a permit held past that bound is still held until its work ends — which is why
//! `GRAPH_HUB_STREAM_DEADLINE_MS` exists.

pub mod permit;
pub mod subscribers;

use std::time::Duration;

use tokio::time::Instant;

use crate::config::Gates;
use crate::gate::permit::Gate;
use crate::gate::subscribers::Subscribers;

pub use permit::{KeyGate, NO_CAP_PERMITS};

/// The four gates and the two subscriber counters, as live objects.
#[derive(Debug, Clone)]
pub struct GateSet {
    /// `GRAPH_HUB_WRITERS`: batches, manifest PUTs and workspace creates at once.
    pub writers: Gate,
    /// `GRAPH_HUB_READS`: `/graph`, `/changes`, records pages, `GET /plugins`, one record and
    /// `/workspaces`.
    pub readers: Gate,
    /// `GRAPH_HUB_LAYOUTS`: `/layout`.
    pub layouts: Gate,
    /// `GRAPH_HUB_WRITERS_PER_KEY`, one permit set per key name.
    pub writers_per_key: KeyGate,
    /// `GRAPH_HUB_MAX_SUBSCRIBERS` and `GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY`.
    pub subscribers: Subscribers,
}

impl GateSet {
    /// The live objects `gates` and `subscribers` describe.
    pub fn new(gates: Gates, subscribers: crate::config::Subscribers) -> Self {
        GateSet {
            writers: Gate::new(gates.writers),
            readers: Gate::new(gates.readers),
            layouts: Gate::new(gates.layouts),
            writers_per_key: KeyGate::new(gates.writers_per_key),
            subscribers: Subscribers::new(subscribers),
        }
    }
}

/// The deadline one wait gets: `Instant::now() + timeout`, where `timeout` is
/// `GRAPH_HUB_TIMEOUT_MS`.
///
/// Caveat: `Instant` rather than a wall clock, because the bound is about how long *this process*
/// has waited and a clock change must not turn a 503 into an unbounded wait.
pub fn deadline(timeout: Duration) -> Instant {
    Instant::now() + timeout
}
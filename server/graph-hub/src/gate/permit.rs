//! The two permits of §6's `WRITERS` and `WRITERS_PER_KEY` rows.
//!
//! [`Gate`] is one semaphore: `GRAPH_HUB_WRITERS`, `GRAPH_HUB_READS` or `GRAPH_HUB_LAYOUTS` slots,
//! and a wait that runs out is a 503 with `Retry-After`, never a 429. [`KeyGate`] is the per-key
//! writer permit, one semaphore per key name held by a `Weak` so an unbounded key count cannot leak.
//!
//! Every number is §6's default. The `no-cap` break turns every limit into [`NO_CAP_PERMITS`], so
//! the 503s disappear at once — which is what row `negctl-no-cap` forces.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

use crate::error::HubApiError;

/// The permits a break turns every gate into.
///
/// Caveat: 64, not `usize::MAX`: `tokio::sync::Semaphore` counts down from a `usize` but a container
/// that asked for 64 permits * 65 536 keys would allocate 64 MiB of counters, and 64 is comfortably
/// above any §6 default while staying a number an operator can reason about.
pub const NO_CAP_PERMITS: usize = 64;

/// A semaphore whose permits are held for as long as the work they admit.
///
/// The permit moves into the work, so a slot frees when the work ends and not when its response is
/// written: for `/graph` and `/layout` that is until the last byte is streamed.
#[derive(Debug, Clone)]
pub struct Gate {
    slots: Arc<Semaphore>,
}

impl Gate {
    /// A gate of `permits` slots, or of [`NO_CAP_PERMITS`] under the `no-cap` break.
    ///
    /// Caveat: nothing checks `permits` against the container's memory; §6's `hub-memory` row is
    /// what measures what a set of concurrent permits costs, and the defaults are sized for 1 GiB.
    pub fn new(permits: usize) -> Self {
        let permits = if crate::breaks::on("no-cap") {
            NO_CAP_PERMITS
        } else {
            permits
        };
        Gate {
            slots: Arc::new(Semaphore::new(permits)),
        }
    }

    /// A permit, now or once one frees before `deadline`.
    ///
    /// On expiry the answer is 503 with `Retry-After: 1`: the request did not find a *queue* full —
    /// nothing here bounds the queue — it waited for a permit past its own budget, and the honest
    /// status is "try again", not "you are too early".
    ///
    /// Caveat: a closed semaphore is a 500, not a 503. Only shutdown closes one, and a request that
    /// arrives then is a hub defect rather than a busy hub.
    pub async fn admit(&self, deadline: Instant) -> Result<OwnedSemaphorePermit, HubApiError> {
        let wait = Arc::clone(&self.slots).acquire_owned();
        match tokio::time::timeout_at(deadline, wait).await {
            Ok(Ok(permit)) => Ok(permit),
            Ok(Err(_)) => Err(HubApiError::Internal("a gate was closed".into())),
            Err(_) => Err(HubApiError::busy_wait()),
        }
    }

    /// Slots free right now, for a log line and for the tests that wait for a release.
    pub fn free(&self) -> usize {
        self.slots.available_permits()
    }
}

/// The per-key writer permit: `GRAPH_HUB_WRITERS_PER_KEY` holds for one key at a time.
///
/// One semaphore per key name, in a `BTreeMap` so a log line that names a gate is in a fixed order.
/// The map holds a `Weak`, so an entry disappears once the last permit is gone: an unbounded number of
/// key names cannot leak, and a key that never writes again costs nothing.
#[derive(Debug, Clone, Default)]
pub struct KeyGate {
    permits: usize,
    entries: Arc<Mutex<BTreeMap<String, Weak<Semaphore>>>>,
}

impl KeyGate {
    /// A gate of `permits` per key name, or of [`NO_CAP_PERMITS`] under the `no-cap` break.
    pub fn new(permits: usize) -> Self {
        KeyGate {
            permits: if crate::breaks::on("no-cap") {
                NO_CAP_PERMITS
            } else {
                permits
            },
            entries: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// A permit for `key`, now or once one frees before `deadline`.
    ///
    /// The map lock is taken to find or make the key's semaphore and released before the wait, so a
    /// request waiting on one key never holds up another key's first permit.
    ///
    /// Caveat: the entry is made *before* the acquire, so two concurrent first requests for one key
    /// share one semaphore instead of racing two. The price is that a refused request leaves an entry
    /// behind until the next call prunes it, which `keys` does.
    pub async fn admit(
        &self,
        key: &str,
        deadline: Instant,
    ) -> Result<OwnedSemaphorePermit, HubApiError> {
        let slots = self.slot_for(key);
        match tokio::time::timeout_at(deadline, slots.acquire_owned()).await {
            Ok(Ok(permit)) => Ok(permit),
            Ok(Err(_)) => Err(HubApiError::Internal("a key gate was closed".into())),
            Err(_) => Err(HubApiError::busy_wait()),
        }
    }

    /// How many key names this gate holds a live entry for, for a log line and for the leak case.
    pub fn keys(&self) -> usize {
        self.live().len()
    }

    /// This key's semaphore, made if there is none.
    fn slot_for(&self, key: &str) -> Arc<Semaphore> {
        let mut entries = self.entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(existing) = entries.get(key).and_then(Weak::upgrade) {
            return existing;
        }
        let slots = Arc::new(Semaphore::new(self.permits));
        entries.insert(key.to_owned(), Arc::downgrade(&slots));
        slots
    }

    /// The entries whose semaphore is still alive, after dropping the dead ones.
    ///
    /// Pruning happens here rather than on every `admit`, so a write-only path never pays for it and
    /// a hub that writes under many key names prunes when the map is next read.
    fn live(&self) -> BTreeMap<String, Weak<Semaphore>> {
        let mut entries = self.entries.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.retain(|_, weak| weak.strong_count() > 0);
        entries.clone()
    }
}

/// The subscriber counters' two numbers, as a lock rather than two atomics.
///
/// One `RwLock<BTreeMap<..>>` rather than two `AtomicUsize`s because the two counters must move
/// together: a per-key count cannot be right if the total was not moved with it, and a `BTreeMap`
/// keeps a log line that names them in key order.
#[derive(Debug)]
pub(crate) struct Counts {
    pub(crate) per_key: BTreeMap<String, usize>,
    pub(crate) total: usize,
}

impl Counts {
    /// No subscriber anywhere.
    pub(crate) fn none() -> Self {
        Counts {
            per_key: BTreeMap::new(),
            total: 0,
        }
    }
}

/// A read lock over the counts, poisoning-tolerant: a panic in a subscriber's task must not make
/// every later stream a 500.
pub(crate) fn read_counts(lock: &RwLock<Counts>) -> std::sync::RwLockReadGuard<'_, Counts> {
    lock.read().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A write lock over the counts, poisoning-tolerant for the same reason.
pub(crate) fn write_counts(lock: &RwLock<Counts>) -> std::sync::RwLockWriteGuard<'_, Counts> {
    lock.write().unwrap_or_else(|poisoned| poisoned.into_inner())
}
//! The two subscriber counters of §6's `MAX_SUBSCRIBERS` pair: the only 429 in the hub.
//!
//! A subscriber is one `GET .../events` stream. The counters are a per-key count and a total, both
//! in one lock, because a per-key count cannot be right if the total was not moved with it.
//!
//! `Subscriber` is the permit: it is returned by [`Subscribers::admit`] and released on drop, so a
//! stream that ends any way — a client's disconnect, a `busy` close, a resync — frees its slot without
//! the stream having to say so. It holds the shared counters rather than the `Subscribers`, so
//! `admit` takes `&self` and a caller does not need an `Arc<Subscribers>` of its own.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::config::Subscribers as Caps;
use crate::error::HubApiError;
use crate::gate::permit::{Counts, read_counts, write_counts};

/// `Retry-After` on a 429, in seconds.
///
/// Caveat: 1 second, the same value every other refusal here carries. §6 gives the caps and not a
/// backoff, and the SDK's own reconnect is jittered (`spec` §7), so this is a floor and not a
/// schedule.
pub const RETRY_AFTER: u64 = 1;

/// The two counters and the cap they enforce.
#[derive(Debug, Clone)]
pub struct Subscribers {
    caps: Caps,
    counts: Arc<RwLock<Counts>>,
}

impl Subscribers {
    /// Counters for `caps`, with each cap through [`crate::config::capped`] so the `no-cap` break
    /// takes the 429 with the rest.
    pub fn new(caps: Caps) -> Self {
        let count = |value: usize| crate::config::capped(value as u64) as usize;
        Subscribers {
            caps: Caps {
                max: count(caps.max),
                per_key: count(caps.per_key),
            },
            counts: Arc::new(RwLock::new(Counts::none())),
        }
    }

    /// A slot for `key`, or a 429 at the per-key cap first and then at the global one.
    ///
    /// The order is per-key then global, and it is the order §6 names: a key that already holds its
    /// eight streams is refused even when the hub has room for fifty-six more from other keys, which
    /// is the point of a per-key cap.
    pub fn admit(&self, key: &str) -> Result<Subscriber, HubApiError> {
        let mut counts = write_counts(&self.counts);
        let mine = counts.per_key.get(key).copied().unwrap_or(0);
        if mine >= self.caps.per_key {
            return Err(HubApiError::busy_subscriber(RETRY_AFTER));
        }
        if counts.total >= self.caps.max {
            return Err(HubApiError::busy_subscriber(RETRY_AFTER));
        }
        counts.per_key.insert(key.to_owned(), mine + 1);
        counts.total += 1;
        Ok(Subscriber {
            owner: Arc::clone(&self.counts),
            key: key.to_owned(),
        })
    }

    /// Give `key`'s slot back. Called by `Subscriber`'s drop, and safe to call twice.
    pub fn release(&self, key: &str) {
        let mut counts = write_counts(&self.counts);
        let mine = counts.per_key.get(key).copied().unwrap_or(0);
        if mine <= 1 {
            counts.per_key.remove(key);
        } else {
            counts.per_key.insert(key.to_owned(), mine - 1);
        }
        counts.total = counts.total.saturating_sub(1);
    }

    /// Streams open right now, in total.
    pub fn total(&self) -> usize {
        read_counts(&self.counts).total
    }

    /// Streams one key holds right now.
    pub fn of(&self, key: &str) -> usize {
        read_counts(&self.counts)
            .per_key
            .get(key)
            .copied()
            .unwrap_or(0)
    }

    /// Every key with a stream open, in key order, for a log line.
    pub fn keys(&self) -> BTreeMap<String, usize> {
        read_counts(&self.counts).per_key.clone()
    }
}

/// One admitted stream, and the slot it gives back when it ends.
#[derive(Debug)]
pub struct Subscriber {
    owner: Arc<RwLock<Counts>>,
    key: String,
}

impl Subscriber {
    /// The key this stream belongs to, for the `event` and log lines.
    pub fn key(&self) -> &str {
        &self.key
    }
}

/// The slot is freed on drop, whatever ended the stream: a client's disconnect, a `busy` close, a
/// resync, or the sweeper's cancellation. A stream that had to say so explicitly could be forgotten
/// by exactly the path that matters most — the one that ended in a failure.
impl Drop for Subscriber {
    fn drop(&mut self) {
        let mut counts = write_counts(&self.owner);
        let mine = counts.per_key.get(&self.key).copied().unwrap_or(0);
        if mine <= 1 {
            counts.per_key.remove(&self.key);
        } else {
            counts.per_key.insert(self.key.clone(), mine - 1);
        }
        counts.total = counts.total.saturating_sub(1);
    }
}

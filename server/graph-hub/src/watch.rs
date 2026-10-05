//! One `tokio::sync::watch` sender per workspace: where a write tells its subscribers that the
//! stream moved.
//!
//! The watch carries a **position** and never a payload: `§5.3`'s subscriber protocol pages the
//! database from its own cursor after every wake-up, so a subscriber that missed a notification
//! still reads every change it missed. That is why the value here is `(epoch, head_seq)` and why
//! the map is never iterated for output order.
//!
//! The rule that makes the stream gapless is H7's monotone one: a publish raises a workspace's
//! position only when the new pair is **greater** than the one in force. A write that applies
//! nothing never publishes, and a second publisher that raced the first cannot move the position
//! backwards.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tokio::sync::watch;

/// One workspace's position, `(epoch, head_seq)`.
pub type Position = (u64, u64);

/// The per-workspace watch registry.
#[derive(Debug, Default)]
pub struct Watch {
    senders: Mutex<BTreeMap<String, watch::Sender<Position>>>,
}

impl Watch {
    /// An empty registry.
    pub fn new() -> Self {
        Watch {
            senders: Mutex::new(BTreeMap::new()),
        }
    }

    /// Raises `ws` to `(epoch, seq)` when that pair is greater than the one in force, and returns
    /// whether it moved.
    ///
    /// Caveat: a workspace with no subscriber still gets an entry, because a subscriber that
    /// connects a moment later must learn the position it missed from the watch and not from a
    /// second read of the workspace row. [`Watch::prune`] drops the entries nothing watches.
    pub fn publish(&self, ws: &str, epoch: u64, seq: u64) -> bool {
        let mut senders = self.lock();
        senders
            .entry(ws.to_owned())
            .or_insert_with(|| watch::channel((epoch, seq)).0)
            .send_if_modified(|held| {
                if epoch > held.0 || (epoch == held.0 && seq > held.1) {
                    *held = (epoch, seq);
                    true
                } else {
                    false
                }
            })
    }

    /// A receiver for `ws`, starting at the position in force right now.
    ///
    /// The subscriber subscribes **before** it pages the database (§5.3's step 1 before step 2), so
    /// a write that lands between the two is seen by the page rather than lost between them.
    pub fn subscribe(&self, ws: &str) -> watch::Receiver<Position> {
        let mut senders = self.lock();
        let sender = senders
            .entry(ws.to_owned())
            .or_insert_with(|| watch::channel((0, 0)).0);
        sender.subscribe()
    }

    /// The position in force for `ws`, or `(0, 0)` when nothing has published one.
    pub fn position(&self, ws: &str) -> Position {
        self.lock().get(ws).map(|s| *s.borrow()).unwrap_or((0, 0))
    }

    /// How many workspaces the registry holds an entry for, for a log line and the leak case.
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Always false: a registry that has published once holds an entry, and a subscriber that has
    /// subscribed creates one. Named because `len` alone trips `clippy::len_without_is_empty`.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Drops the entries no subscriber is watching, so a workspace that is written once and never
    /// subscribed to does not stay for the hub's life.
    ///
    /// Caveat: this runs on publish, so an entry survives until the next write to any workspace. A
    /// hub that writes to millions of workspaces and never subscribes still holds one entry each
    /// until then; the bound is §6's `LAST_SEEN`, and this is the same shape as that map's.
    pub fn prune(&self) {
        self.lock().retain(|_, sender| sender.receiver_count() > 0);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, watch::Sender<Position>>> {
        self.senders
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// The registry as an `Arc`, which is how [`crate::app::App`] holds it.
pub type Registry = Arc<Watch>;

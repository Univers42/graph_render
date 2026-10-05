//! The connection pool and the restore detector that runs on every new connection.

pub mod connect;
pub mod detect;
pub mod last_seen;

use std::sync::Mutex;

use crate::error::StoreError;

pub use last_seen::LastSeen;

/// What the detector decided about a database it has just connected to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectorOutcome {
    /// Nothing about the database contradicts what this process last committed.
    Match,
    /// The database was restored, promoted or replaced; every workspace drew a fresh epoch.
    Bumped {
        /// How many workspaces the bump touched.
        workspaces: u64,
    },
}

/// The restore detector's state, shared by every connection in the pool.
///
/// §5.3: the high-water is the highest `pg_current_wal_flush_lsn()` this process has read after
/// a commit, and the last-seen map is the highest `(epoch, head_seq)` per workspace. Both live
/// in memory only; the upgrade path persists them outside the database (a Caveat of record in the
/// spec).
///
/// The mutex is held from before the high-water is snapshotted until after it is raised again,
/// which is what stops two connections opening at once from both deciding to bump.
#[derive(Debug)]
pub struct Detector {
    /// Held from before the snapshot until after the high-water is raised again (S2).
    ///
    /// `tokio::sync::Mutex`, not `std::sync::Mutex`: the guard is held across `await` points, and
    /// a `std` guard there would make the future non-`Send`, which every caller that is awaited
    /// inside a spawned task requires.
    guard: tokio::sync::Mutex<()>,
    /// The snapshotted high-water, `None` before this process has ever committed.
    high_water: Mutex<Option<String>>,
    /// The last-seen map. Never a `HashMap`: the §5.3 order is fixed and eviction is by recency.
    seen: Mutex<LastSeen>,
    /// `detector-at-start` reads this: with the break on, only the first connection is checked.
    started: std::sync::atomic::AtomicBool,
}

impl Detector {
    /// A detector with no high-water and an empty map, sized for `cap` entries.
    pub fn new(cap: usize) -> Detector {
        Detector {
            guard: tokio::sync::Mutex::new(()),
            high_water: Mutex::new(None),
            seen: Mutex::new(LastSeen::new(cap)),
            started: std::sync::atomic::AtomicBool::new(false),
        }
    }

    /// The lock held across one detector run.
    pub(crate) async fn guard(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.guard.lock().await
    }

    /// Claim the right to run the detector, unless `detector-at-start` has already spent it.
    ///
    /// The break replaces "every new connection" with "the first one", which is what S1 forbids.
    pub(crate) fn claim_run(&self) -> bool {
        if !crate::breaks::on("detector-at-start") {
            return true;
        }
        !self.started.swap(true, std::sync::atomic::Ordering::SeqCst)
    }

    /// The snapshotted high-water, or `None` before this process has committed.
    pub fn high_water(&self) -> Option<String> {
        match self.high_water.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    /// Raise the high-water to `lsn` alone, after a bump has committed.
    pub fn set_high_water(&self, lsn: &str) {
        if let Ok(mut guard) = self.high_water.lock() {
            *guard = Some(lsn.to_string());
        }
    }

    /// Raise the high-water to `lsn` and record `(epoch, seq)` for `ws`, after the transaction
    /// that produced them has committed.
    ///
    /// Called on the *same connection* as the commit, in §5.1 step 8. Entries only rise, except
    /// after a bump, when the fresh epoch is by definition above every entry before it.
    pub fn commit_watermark(&self, lsn: &str, ws: &str, epoch: u64, seq: u64) {
        if let Ok(mut guard) = self.high_water.lock() {
            *guard = Some(lsn.to_string());
        }
        if let Ok(mut guard) = self.seen.lock() {
            guard.raise(ws, epoch, seq);
        }
    }

    /// The high-water and the last-seen map, snapshotted together before any read.
    ///
    /// §5.3 step 2 requires both to be read *before* the LSN and the workspace rows, so that a
    /// write landing between the two reads cannot be mistaken for a restore.
    pub fn snapshot(
        &self,
    ) -> (
        Option<String>,
        std::collections::BTreeMap<String, (u64, u64)>,
    ) {
        let high = self.high_water();
        let seen = match self.seen.lock() {
            Ok(mut guard) => guard.snapshot(),
            Err(poisoned) => poisoned.into_inner().snapshot(),
        };
        (high, seen)
    }

    /// Forget the high-water and the map, after a bump has committed.
    ///
    /// A *failed* commit leaves both alone, so the next connection bumps again: that is the
    /// point of clearing only here.
    pub fn clear(&self) {
        if let Ok(mut guard) = self.high_water.lock() {
            *guard = None;
        }
        if let Ok(mut guard) = self.seen.lock() {
            guard.clear();
        }
    }

    /// Run the detector on `client`, in §5.3's order.
    pub async fn run(
        &self,
        client: &mut tokio_postgres::Client,
    ) -> Result<DetectorOutcome, StoreError> {
        connect::run_detector(self, client).await
    }
}

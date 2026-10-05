//! The store: the one object a caller holds, and the only way to reach the database.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use graph_contract::hub::Limits;

use crate::config::StoreConfig;
use crate::error::StoreError;
use crate::pool::Detector;
use crate::writer::{BatchOutcome, BatchWrite, ManifestWrite, ManifestWritten};

/// A connected store.
///
/// Holds a detector shared by every connection, so a restore is noticed by whichever connection
/// opens next rather than by one that happens to be free.
#[derive(Debug, Clone)]
pub struct Store {
    /// The `postgres://` URL, kept so the sweeper and a reconnect can reuse it.
    url: String,
    /// The per-workspace restore detector, shared across the pool.
    detector: Arc<Detector>,
    /// The limits and defaults this store was opened with.
    config: Arc<StoreConfig>,
    /// How many times §5.1's one retry has run, so a row can assert that it did.
    retries: Arc<AtomicU64>,
}

impl Store {
    /// Open a store on `cfg.url`, running the detector on the first connection.
    pub async fn connect(cfg: &StoreConfig) -> Result<Store, StoreError> {
        crate::config::check(cfg)?;
        if cfg.url.is_empty() {
            return Err(StoreError::NoDatabase);
        }
        let detector = Arc::new(Detector::new(cfg.last_seen as usize));
        Ok(Store {
            url: cfg.url.clone(),
            detector,
            config: Arc::new(cfg.clone()),
            retries: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Prove the database answers.
    pub async fn ping(&self) -> Result<(), StoreError> {
        let client = self.client().await?;
        client.query_one("SELECT 1", &[]).await?;
        Ok(())
    }

    /// One connection from this store, with the detector already run on it.
    pub async fn client(&self) -> Result<tokio_postgres::Client, StoreError> {
        crate::pool::connect::open(&self.url, &self.detector).await
    }

    /// The detector this store's connections share.
    pub fn detector(&self) -> &Arc<Detector> {
        &self.detector
    }

    /// The configuration this store was opened with.
    pub fn config(&self) -> &Arc<StoreConfig> {
        &self.config
    }

    /// §4's workspace create: `true` when the row was inserted (201), `false` when it already
    /// existed (200). It takes no seq — a workspace has no change log of its own.
    pub async fn create_workspace(
        &self,
        ws: &str,
        limits: &Limits,
    ) -> Result<bool, StoreError> {
        crate::writer::create_workspace(self, ws, limits).await
    }

    /// §4's manifest PUT: growth check, the 64-plugin cap, the document byte cap, and a change of
    /// kind `manifest` when the manifest actually grew.
    pub async fn put_manifest(&self, req: &ManifestWrite) -> Result<ManifestWritten, StoreError> {
        crate::writer::put_manifest(self, req).await
    }

    /// §5.1's batch, in its eight steps.
    pub async fn apply_batch(&self, req: &BatchWrite) -> Result<BatchOutcome, StoreError> {
        crate::writer::apply_batch(self, req).await
    }

    /// How many times the one permitted retry has run on this store.
    ///
    /// It is a counter and not a flag because a workspace is written by many batches and only the
    /// sum is a fact a row can assert: `a_deadlock_ends_in_a_commit_or_a_503_never_a_500` reads it
    /// after a forced deadlock, and `no-deadlock-retry` has to leave it at zero.
    pub fn retry_count(&self) -> u64 {
        self.retries.load(Ordering::Relaxed)
    }

    /// Record that the one retry ran. Called by [`crate::writer::retry`], nowhere else.
    pub(crate) fn note_retry(&self) {
        self.retries.fetch_add(1, Ordering::Relaxed);
    }
}
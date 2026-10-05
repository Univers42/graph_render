//! The store: the one object a caller holds, and the only way to reach the database.

use std::sync::Arc;

use crate::config::StoreConfig;
use crate::error::StoreError;
use crate::pool::Detector;

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
}

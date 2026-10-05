//! The store's configuration: every §6 default in one place, and the start checks that refuse a
//! database the contract cannot be honoured on.
//!
//! `defaults()` spells every default out as a literal, so a spec change that renames or moves a
//! default is a failing test (`config_defaults_match_section_6`) rather than a silent behaviour
//! change.

/// Everything `Store::connect` reads. `defaults()` carries every §6 default except `url`, which
/// has none: it is the one field a caller must fill in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreConfig {
    /// The `postgres://` URL of the hub database. Empty in `defaults()`.
    pub url: String,
    /// `GRAPH_HUB_DB_POOL`: connections in the pool. Default 8.
    pub pool: u64,
    /// `GRAPH_HUB_RETAIN`: changes kept per workspace. Default 100 000.
    pub retain: u64,
    /// `GRAPH_HUB_RETAIN_BYTES`: bytes of change log kept per workspace. Default 512 MiB.
    pub retain_bytes: u64,
    /// `GRAPH_HUB_CHANGES_BYTES`: bytes one `/changes` page may carry. Default 8 MiB.
    pub changes_bytes: u64,
    /// `GRAPH_HUB_FETCH_ROWS`: rows one portal page reads. Default 32.
    pub fetch_rows: u64,
    /// `GRAPH_HUB_MAX_BATCH`: operations in one batch. Default 10 000.
    pub max_batch: u64,
    /// `GRAPH_HUB_MAX_BODY`: bytes in one request body. Default 4 MiB.
    pub max_body: u64,
    /// `GRAPH_HUB_MAX_RECORD_BYTES`: bytes in one record's canonical text. Default 1 MiB.
    pub max_record_bytes: u64,
    /// `GRAPH_HUB_MAX_PLUGIN_BYTES`: bytes one plugin's records may reach. Default 16 MiB.
    pub max_plugin_bytes: u64,
    /// `GRAPH_HUB_MAX_DOC_BYTES`: bytes one workspace's document may reach. Default 64 MiB.
    pub max_doc_bytes: u64,
    /// `GRAPH_HUB_LAST_SEEN`: entries in the detector's last-seen map. Default 65 536.
    pub last_seen: u64,
    /// `GRAPH_HUB_TIMEOUT_MS`: a pool wait before 503. Default 30 000.
    pub timeout_ms: u64,
    /// `GRAPH_HUB_STREAM_DEADLINE_MS`: a document stream's bound. Default 120 000.
    pub stream_deadline_ms: u64,
    /// The idempotency sweeper's interval. Default 600 000 (10 minutes).
    pub sweeper_interval_ms: u64,
    /// `GRAPH_HUB_IDEM_TTL_MS`: how long an idempotency row lives. Default 86 400 000 (24 h).
    pub idem_ttl_ms: u64,
}

impl Default for StoreConfig {
    fn default() -> Self {
        StoreConfig::defaults()
    }
}

impl StoreConfig {
    /// Every §6 default spelled out, with `url` empty.
    pub fn defaults() -> StoreConfig {
        StoreConfig {
            url: String::new(),
            pool: 8,
            retain: 100_000,
            retain_bytes: 512 * 1024 * 1024,
            changes_bytes: 8 * 1024 * 1024,
            fetch_rows: 32,
            max_batch: 10_000,
            max_body: 4 * 1024 * 1024,
            max_record_bytes: 1024 * 1024,
            max_plugin_bytes: 16 * 1024 * 1024,
            max_doc_bytes: 64 * 1024 * 1024,
            last_seen: 65_536,
            timeout_ms: 30_000,
            stream_deadline_ms: 120_000,
            sweeper_interval_ms: 600_000,
            idem_ttl_ms: 86_400_000,
        }
    }
}

/// The §6 limits a `Limits` needs, read off a config so the store never takes nine arguments.
impl StoreConfig {
    /// The three caps `graph_contract::hub::Limits` carries.
    pub fn limits(&self) -> graph_contract::hub::Limits {
        graph_contract::hub::Limits {
            max_body: self.max_body,
            max_batch: self.max_batch,
            max_record_bytes: self.max_record_bytes,
        }
    }

    /// The largest one change may be: `max_change` in graph-contract, recomputed here so the
    /// §6 start check can compare it against the page and retention bounds.
    pub fn max_change(&self) -> u64 {
        graph_contract::hub::max_change(&self.limits())
    }
}

/// The §6 start checks that do not need a database: the two byte bounds must hold one maximum
/// change each, or a single change could exceed the bound that was supposed to hold it.
///
/// Returns the first violation, or `Ok(())`.
pub fn check(cfg: &StoreConfig) -> Result<(), crate::error::StoreError> {
    let max = cfg.max_change();
    if cfg.retain_bytes < max {
        let message = format!("retain_bytes {} < max_change {max}", cfg.retain_bytes);
        return Err(crate::error::StoreError::Db(
            crate::error::DbError::store("XX001", message),
        ));
    }
    if cfg.changes_bytes < max {
        let message = format!("changes_bytes {} < max_change {max}", cfg.changes_bytes);
        return Err(crate::error::StoreError::Db(
            crate::error::DbError::store("XX001", message),
        ));
    }
    Ok(())
}
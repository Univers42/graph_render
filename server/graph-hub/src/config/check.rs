//! §6's start checks, in the order §6 gives them, each naming the variable that is wrong.
//!
//! Five need no connection and run before the listener is bound; two read the database and run
//! after [`graph_store::Store::ping`] answers. A refusal is a [`ConfigError`], so `main` prints
//! `name: reason` and exits 2 the way graph-server's `main.rs:35-38` does.
//!
//! The `no-start-check` break makes [`Settings::check`] return `Ok(())` after logging what it
//! skipped, which is what row `negctl-no-start-check` forces: a green `start_check_refuses_*` can
//! then never be an unchecked `check`.

use graph_store::Store;

use crate::breaks;
use crate::config::{ConfigError, Settings};

impl Settings {
    /// Every §6 refusal, or `Ok(())`.
    ///
    /// `async` because two of the seven read the database, and `db: &Store` rather than a URL
    /// because the connection is the store's own: the restore detector has already run on it
    /// (§5.3), so a refusal here is about the deployment and not about a query.
    ///
    /// Caveat: this is a start check, not a runtime guard: nothing re-reads the variables, so a
    /// value that is legal here stays legal for the life of the process. The escape hatch for a
    /// wrong deployment is the variable itself.
    pub async fn check(&self, db: &Store) -> Result<(), ConfigError> {
        self.check_numbers()?;
        self.check_pool()?;
        if breaks::on("no-start-check") {
            self.log_skipped_checks();
            return Ok(());
        }
        self.check_pages()?;
        self.check_motor()?;
        check_database(db).await
    }

    /// (1) `GRAPH_HUB_DB_URL` empty, and (2) either credential file unset (Decision 7).
    fn check_numbers(&self) -> Result<(), ConfigError> {
        if self.db_url.is_empty() {
            return Err(ConfigError::new("GRAPH_HUB_DB_URL", "is unset or empty"));
        }
        for (name, path) in [
            ("GRAPH_HUB_KEYS_FILE", &self.keys_file),
            ("GRAPH_HUB_GRANTS_FILE", &self.grants_file),
        ] {
            if path.as_os_str().is_empty() {
                let reason = "is unset or empty";
                return Err(ConfigError::new(name, reason));
            }
        }
        Ok(())
    }

    /// (3) `GRAPH_HUB_DB_POOL <= WRITERS + READS + LAYOUTS`.
    ///
    /// A pool at or below the permits is a refusal rather than a warning because every permit can
    /// be held at once, and a hub whose permits cannot all be admitted deadlocks its own writes
    /// against its own reads. SSE header pages and the sweeper need the connections that are left.
    fn check_pool(&self) -> Result<(), ConfigError> {
        let permits = self.gates.writers + self.gates.readers + self.gates.layouts;
        if self.store.pool as usize <= permits {
            let reason = "must exceed GRAPH_HUB_WRITERS + GRAPH_HUB_READS + GRAPH_HUB_LAYOUTS";
            return Err(ConfigError::new("GRAPH_HUB_DB_POOL", reason));
        }
        Ok(())
    }

    /// (4) `GRAPH_HUB_RETAIN_BYTES` and `GRAPH_HUB_CHANGES_BYTES` must each hold one maximum
    /// change.
    ///
    /// Both are refusals for the same reason: a bound that cannot hold one change is not a bound.
    /// The maximum is graph-contract's `max_change` over `Limits::DEFAULT`, recomputed here through
    /// [`graph_store::StoreConfig::max_change`], so the store's own `check` and this one compare
    /// the same number.
    fn check_pages(&self) -> Result<(), ConfigError> {
        let max = self.store.max_change();
        if self.limits.retain_bytes < max {
            let reason = "is below one maximum change";
            return Err(ConfigError::new("GRAPH_HUB_RETAIN_BYTES", reason));
        }
        if self.limits.changes_bytes < max {
            let reason = "is below one maximum change";
            return Err(ConfigError::new("GRAPH_HUB_CHANGES_BYTES", reason));
        }
        Ok(())
    }

    /// (5) `GRAPH_HUB_MOTOR_TIMEOUT_MS <= 40_000`.
    ///
    /// The floor is graph-server's `GRAPH_TIMEOUT_MS` plus its `GRAPH_BODY_TIMEOUT_MS`
    /// (30 000 + 10 000) rounded down: below that the hub gives up on the motor while the motor is
    /// still working, and the answer would be a 502 for a request that was about to succeed.
    fn check_motor(&self) -> Result<(), ConfigError> {
        if self.motor_timeout.as_millis() as u64 <= 40_000 {
            let reason = "must exceed 40000, the motor's own timeout plus its body timeout";
            return Err(ConfigError::new("GRAPH_HUB_MOTOR_TIMEOUT_MS", reason));
        }
        Ok(())
    }

    /// What the `no-start-check` break skipped, as one log line. The break is the only caller: a
    /// shipped build never reaches this.
    fn log_skipped_checks(&self) {
        let skipped = serde_json::json!({
            "event": "start-check",
            "skipped": ["GRAPH_HUB_RETAIN_BYTES", "GRAPH_HUB_CHANGES_BYTES", "GRAPH_HUB_MOTOR_TIMEOUT_MS"],
            "break": "no-start-check",
            "pool": self.store.pool,
            "permits": self.gates.writers + self.gates.readers + self.gates.layouts,
        });
        eprintln!("graph-hub: {skipped}");
    }
}

/// (6) `server_encoding <> 'UTF8'` and (7) `pg_database.datcollate <> 'C'`, over one live
/// connection.
///
/// The image runs `initdb --encoding=UTF8 --locale=C`, so `COLLATE "C"` order is UTF-8 byte order
/// and every scan in §5.3's document order is byte order. A database on another locale would make
/// `/graph` and the records pages order their output by the database's rules instead.
///
/// The checks are the store's connection, so the restore detector has already run on it (§5.3), and
/// a refusal here is a refusal of the deployment rather than of the query.
pub async fn check_database(db: &Store) -> Result<(), ConfigError> {
    let client = db
        .client()
        .await
        .map_err(|_| ConfigError::new("GRAPH_HUB_DB_URL", "cannot be reached"))?;
    let encoding: String = client
        .query_one("SELECT pg_encoding_to_char(encoding) FROM pg_database \
                    WHERE datname = current_database()",
                   &[])
        .await
        .map_err(|_| ConfigError::new("GRAPH_HUB_DB_URL", "cannot be read"))?
        .get(0);
    if encoding != "UTF8" {
        let reason = "the database's encoding is not UTF8";
        return Err(ConfigError::new("GRAPH_HUB_DB_URL", reason));
    }
    let collation: String = client
        .query_one("SELECT datcollate FROM pg_database WHERE datname = current_database()", &[])
        .await
        .map_err(|_| ConfigError::new("GRAPH_HUB_DB_URL", "cannot be read"))?
        .get(0);
    if collation != "C" {
        let reason = "the database's collation is not C, so byte order is not its order";
        return Err(ConfigError::new("GRAPH_HUB_DB_URL", reason));
    }
    Ok(())
}
//! The state every request shares: the log sink and the test seams. Built once at start; nothing
//! in it changes per request. Task 2 adds the settings, Task 3 the keyring, Task 4 the gates, and
//! Task 7 the watch registry, one field at a time, so each task's diff is that task's own state.

use std::sync::Arc;

use graph_store::Store;

use crate::config::Settings;
use crate::error::HubApiError;
use crate::gate::GateSet;
use crate::hooks::Hooks;
use crate::keys::Keyring;

/// Where one JSON log line goes: stdout in the binary, a buffer in tests.
pub type LogSink = Arc<dyn Fn(&str) + Send + Sync>;

/// The shared state.
pub struct App {
    /// The log line sink.
    pub log: LogSink,
    /// §6's settings, read once at start.
    pub settings: Settings,
    /// The credential pair: the key set and the grants, swapped together on `SIGHUP`.
    pub keys: Keyring,
    /// The four semaphores and the two subscriber counters, built from the settings once.
    pub gates: GateSet,
    /// The test seams; empty unless the `test-hooks` feature is on.
    pub hooks: Hooks,
    /// One watch sender per workspace, where a write says the stream moved.
    pub watch: crate::watch::Watch,
    /// The store every handler reads and writes through, opened once and shared.
    ///
    /// Caveat: this is a `OnceCell` and not a plain field because [`App::from_settings`] is
    /// synchronous and `Store::connect` is not. The binary primes the cell with the store it
    /// already opened for the start checks ([`App::set_store`]); a test that never asked the
    /// settings for a database never opens one at all.
    store: tokio::sync::OnceCell<Store>,
}

impl App {
    /// The state `settings` and `log` describe, reading both credential files.
    ///
    /// The refusal is a message for the start path and never holds a line of either file: both are
    /// credentials, and `KeySet::load`'s own error names a line number and a reason only.
    pub fn from_settings(settings: &Settings, log: LogSink) -> Result<Arc<App>, String> {
        let keys = Keyring::load(&settings.keys_file, &settings.grants_file)?;
        let gates = GateSet::new(settings.gates, settings.subscribers);
        Ok(Arc::new(Self {
            log,
            settings: settings.clone(),
            keys,
            gates,
            hooks: Hooks::new(),
            watch: crate::watch::Watch::new(),
            store: tokio::sync::OnceCell::new(),
        }))
    }

    /// The store, opened from the settings' own `GRAPH_HUB_DB_URL` the first time a handler asks.
    ///
    /// Caveat: the store is opened lazily, so a hub whose `GRAPH_HUB_DB_URL` is wrong refuses at
    /// the first `/v1` request rather than at start. The binary does not rely on that path: `main`
    /// opens the store before the start checks and hands it over with [`App::set_store`], so a
    /// shipped hub has already answered that refusal at start.
    pub async fn store(&self) -> Result<&Store, HubApiError> {
        self.store
            .get_or_try_init(|| async {
                let mut config = self.settings.store.clone();
                config.url = self.settings.db_url.clone();
                Store::connect(&config).await
            })
            .await
            .map_err(|error| {
                HubApiError::Internal(format!("the store is unusable: {}", error.code()))
            })
    }

    /// Hands the hub the store the binary already opened, so the shipped process never opens two.
    ///
    /// `Err` is the store back, which happens only when a second call raced the first; the caller
    /// treats it as a hub defect because `main` calls this once.
    pub fn set_store(&self, store: Store) -> Result<(), String> {
        self.store
            .set(store)
            .map_err(|_| String::from("the store was already set"))
    }

    /// Writes one JSON log line.
    pub fn log(&self, line: &serde_json::Value) {
        (self.log)(&line.to_string());
    }

    /// The token of one `Authorization` header value, or `None` when it is not a `Bearer`
    /// credential.
    ///
    /// This is the hub's **only** use of the compute crate's credential parsing: it reuses
    /// `graph_server::auth::bearer` (`docs/decisions/graph-hub.md:120-121`) and nothing else of
    /// that module, because its credential check carries the `any-key` break and the compute
    /// `App` (`server/graph-server/src/auth.rs:24`). Task 3 moves the decision into
    /// `auth::credential`, which calls this.
    ///
    /// Caveat: RFC 6750 §2.1 leaves the scheme case-insensitive and the token's own syntax open,
    /// so this accepts any non-empty token and the key file's hash is what decides.
    pub fn credential_of(value: &str) -> Option<&str> {
        graph_server::auth::bearer(value)
    }
}

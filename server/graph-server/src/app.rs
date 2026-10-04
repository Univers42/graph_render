//! The state every request shares: the key store, the limits, the admission gate, the caps,
//! the embed tree and the log sink. Built once at start from [`Settings`]; nothing in it
//! changes per request except the key set on `SIGHUP`.

use crate::caps::Caps;
use crate::config::{Limits, Settings};
use crate::embed::Embed;
use crate::gate::Gate;
use crate::keys::{KeySet, KeyStore};
use crate::observe::RequestIds;
use std::path::Path;
use std::sync::Arc;

/// Where one JSON log line goes: stdout in the binary, a buffer in tests.
pub type LogSink = Arc<dyn Fn(&str) + Send + Sync>;

/// The shared state.
pub struct App {
    /// The live keys; `None` when `GRAPH_AUTH=off`.
    pub keys: Option<KeyStore>,
    /// The compute limits.
    pub limits: Limits,
    /// The log line sink.
    pub log: LogSink,
    /// The compute slots and their queue.
    pub gate: Gate,
    /// The per-id work caps.
    pub caps: Caps,
    /// The embed tree; `None` when `GRAPH_EMBED_DIR` is unset, so every `/embed/` path is 404.
    pub embed: Option<Embed>,
    /// The origins allowed on `/v1/`.
    pub cors_origins: Vec<String>,
    /// Generated request ids.
    pub ids: RequestIds,
    /// Test seams; absent unless the `test-hooks` feature is on.
    #[cfg(feature = "test-hooks")]
    pub hooks: Hooks,
}

/// Code the tests run inside a request, to make a run slow or make it panic. Compiled only with
/// the `test-hooks` feature, which the crate's dev-dependency on itself turns on for the tests and
/// no build of the binary turns on at all (row `hooks-gated`).
#[cfg(feature = "test-hooks")]
#[derive(Default)]
pub struct Hooks {
    /// Called on the blocking thread before the motor builds the graph.
    pub before_run: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl App {
    /// The state for `settings`, reading the key file. The error is a message for the start
    /// refusal and never holds a line of the key file.
    pub fn from_settings(settings: &Settings, log: LogSink) -> Result<Self, String> {
        let keys = match (&settings.keys_file, settings.auth) {
            (Some(path), true) => Some(KeyStore::new(
                KeySet::load(path).map_err(|e| e.to_string())?,
            )),
            _ => None,
        };
        let embed = match &settings.embed_dir {
            Some(dir) => Some(Embed::load(dir)?),
            None => None,
        };
        let limits = settings.limits;
        Ok(Self {
            keys,
            limits,
            log,
            gate: Gate::new(limits.workers, limits.queue),
            caps: Caps::committed()?,
            embed,
            cors_origins: settings.cors_origins.clone(),
            ids: RequestIds::new()?,
            #[cfg(feature = "test-hooks")]
            hooks: Hooks::default(),
        })
    }

    /// Writes one JSON log line.
    pub fn log(&self, line: &serde_json::Value) {
        (self.log)(&line.to_string());
    }

    /// `SIGHUP`: parses the whole file, then swaps. A bad or empty file keeps the old set.
    pub fn reload_keys(&self, path: Option<&Path>) {
        let (Some(store), Some(path)) = (&self.keys, path) else {
            self.log(&serde_json::json!({ "event": "keys", "reload": "auth is off" }));
            return;
        };
        match KeySet::load(path) {
            Ok(keys) => {
                let count = keys.len();
                store.replace(keys);
                self.log(&serde_json::json!({ "event": "keys", "reloaded": count }));
            }
            Err(refused) => {
                let kept = store.current().len();
                let reason = refused.to_string();
                self.log(&serde_json::json!({ "event": "keys", "refused": reason, "kept": kept }));
            }
        }
    }
}

//! The state every request shares: the key store, the limits and the log sink. Built once at
//! start from [`Settings`]; nothing in it changes per request except the key set on `SIGHUP`.

use crate::config::{Limits, Settings};
use crate::keys::{KeySet, KeyStore};
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
        Ok(Self {
            keys,
            limits: settings.limits,
            log,
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

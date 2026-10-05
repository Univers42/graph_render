//! The state every request shares: the log sink and the test seams. Built once at start; nothing
//! in it changes per request. Task 2 adds the settings, Task 3 the keyring, Task 4 the gates, and
//! Task 7 the watch registry, one field at a time, so each task's diff is that task's own state.

use std::sync::Arc;

use crate::hooks::Hooks;

/// Where one JSON log line goes: stdout in the binary, a buffer in tests.
pub type LogSink = Arc<dyn Fn(&str) + Send + Sync>;

/// The shared state.
pub struct App {
    /// The log line sink.
    pub log: LogSink,
    /// The test seams; empty unless the `test-hooks` feature is on.
    pub hooks: Hooks,
}

impl App {
    /// The state with nothing but a log sink, which is all the two routes of Task 1 need.
    pub fn new(log: LogSink) -> Arc<App> {
        Arc::new(Self {
            log,
            hooks: Hooks::default(),
        })
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
    /// `auth`, because `auth::check` carries the `any-key` break and the compute `App`
    /// (`auth.rs:24`). Task 3 moves the decision into `auth::credential`, which calls this.
    ///
    /// Caveat: RFC 6750 §2.1 leaves the scheme case-insensitive and the token's own syntax open,
    /// so this accepts any non-empty token and the key file's hash is what decides.
    pub fn credential_of(value: &str) -> Option<&str> {
        graph_server::auth::bearer(value)
    }
}
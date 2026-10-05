//! Where a JSON log line goes: stdout in the binary, a buffer in the tests. The sink type is the
//! hub's own (`server/graph-server/src/app.rs:15` is the same shape), because `graph-server` is a
//! dependency for `auth::bearer` and `keys::KeySet` and nothing else.

use std::io::Write;
use std::sync::Arc;

use crate::app::LogSink;

/// The stdout sink: one line per call, and a write failure is ignored, because a log line that
/// cannot be written must not end the process.
pub fn stdout_sink() -> LogSink {
    Arc::new(|line: &str| {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{line}");
    })
}
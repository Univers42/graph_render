//! Logging: one JSON line per event on stdout, the only place the service writes
//! (`docs/contract/service-api.md` "The image": no disk writes).

use crate::app::LogSink;
use std::io::Write;
use std::sync::Arc;

/// The binary's sink: one line on stdout, under the stdout lock so lines never interleave.
pub fn stdout_sink() -> LogSink {
    Arc::new(|line: &str| {
        // A closed stdout leaves nowhere to report the failure; the request is still served.
        let _ = writeln!(std::io::stdout().lock(), "{line}");
    })
}

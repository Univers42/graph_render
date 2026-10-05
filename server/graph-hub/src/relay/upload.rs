//! What one `/layout` upload cost the hub: the bytes and records it streamed, and the single
//! `layout-upload` line it writes when the last chunk is out.
//!
//! The line is what `docs/measurements/hub-memory.md` reads to time the upload against
//! graph-server's `GRAPH_BODY_TIMEOUT_MS` (Decision 4), and the measurement reads **six** of them
//! per run: one warm-up and five timed. So the line counts bytes and records as the walk yields
//! them rather than deriving either from a chunk count, and `upload_ms` runs from the first poll of
//! the body stream to the yield of the tail — the instant the last chunk is handed to the socket,
//! not the motor's answer.
//!
//! Caveat: the instant is taken when the walk is first polled, not when `send` is called, so it
//! includes the hub's own scheduling before the first chunk leaves. On loopback between two
//! containers that is small beside the 64 MiB the walk writes, and it is the honest end of "the
//! hub's streaming cost" — a start earlier than the first poll would measure a socket the walk
//! never used.
//!
//! Caveat: a stream dropped before its tail is over — a motor that answered 408 mid-upload, or a
//! caller that hung up — writes **no** line, because there is no completed upload to name. A
//! count of lines is therefore a count of uploads that reached their tail, which is what the
//! measurement's six are.

use serde_json::json;

use crate::app::LogSink;

/// One upload's accounting, held by the walk and dropped with it.
pub struct Upload {
    log: LogSink,
    ws: String,
    start: Option<tokio::time::Instant>,
    bytes: u64,
    records: u64,
    logged: bool,
}

impl Upload {
    /// The accounting for `ws`'s upload, logged through `log`.
    pub fn new(log: LogSink, ws: &str) -> Self {
        Self {
            log,
            ws: ws.to_owned(),
            start: None,
            bytes: 0,
            records: 0,
            logged: false,
        }
    }

    /// The walk's first poll: the clock starts here and not before.
    pub fn begin(&mut self) {
        if self.start.is_none() {
            self.start = Some(tokio::time::Instant::now());
        }
    }

    /// One chunk of `bytes` yielded.
    pub fn wrote(&mut self, bytes: usize) {
        self.bytes += bytes as u64;
    }

    /// One record yielded.
    pub fn recorded(&mut self) {
        self.records += 1;
    }

    /// The tail is out: write the line, once.
    ///
    /// The `logged` guard is what makes the line once per upload rather than once per path through
    /// the stage machine, and the `logged = true` is set **before** the sink is called so a sink
    /// that logs into the walk cannot re-enter it.
    pub fn finish(&mut self) {
        if self.logged {
            return;
        }
        self.logged = true;
        let Some(start) = self.start else {
            return;
        };
        let line = json!({
            "event": "layout-upload",
            "ws": self.ws,
            "bytes": self.bytes,
            "records": self.records,
            "upload_ms": millis(start.elapsed()),
        });
        (self.log)(&line.to_string());
    }
}

/// Elapsed milliseconds as the `u64` the line carries, saturating rather than wrapping.
///
/// A saturating read and not `as u64`: the value is a duration and a duration above 584 million
/// years is a broken clock, not a number a truncation should turn into a small one.
fn millis(elapsed: std::time::Duration) -> u64 {
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

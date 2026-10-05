//! §5.3's notice stream: `GET /v1/workspaces/{ws}/events`.
//!
//! Every case here is about the stream's *ends* rather than its middle, because the middle is the
//! store's change feed and `/changes` already covers it. What the hub owns is the resume cursor,
//! the two ways a stream stops early (`busy` and `resync`), the slot the subscriber holds while it
//! runs, and the heartbeat.
//!
//! The four tests the plan's Review Focus 3 names are here by name:
//! `busy_carries_no_id_line`, `the_busy_slot_is_free_before_the_close`,
//! `the_busy_reconnect_reads_no_graph` and `a_cursor_pruned_during_the_backoff_gets_resync`.
#![cfg(feature = "db-tests")]

#[path = "events/ends.rs"]
mod ends;
#[path = "events/stream.rs"]
mod stream;
mod support;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;

use support::db;

/// A TCP listener on loopback and the URL of a hub that serves the given router.
///
/// WHY a socket and not `oneshot`: an SSE stream is infinite, so a test has to read a bounded prefix
/// of it and hang up, and `hyper`'s in-process body cannot be read "until the server closes" without
/// a driver that the tests would each write. A socket gives the raw bytes §5.3's wire text is made
/// of, which is also what `busy_carries_no_id_line` has to assert on.
///
/// Caveat: this binds a second port per case, so a case is one process-level listener; §6's
/// `MAX_CONNECTIONS` is not what is under test here and the hub serves this connection directly.
pub async fn serve(router: axum::Router) -> String {
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("a loopback port");
    let port = listener.local_addr().expect("the bound address").port();
    tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    format!("http://127.0.0.1:{port}")
}

/// One SSE request's raw bytes, up to `lines` lines or the close, whichever comes first.
///
/// The read is bounded by [`PATIENCE`]: a stream that produces nothing must fail the case rather
/// than hang the suite, and a stream that produces everything closes and ends the read on its own.
pub async fn raw_events(url: &str, path: &str, key: &str, lines: usize) -> Vec<String> {
    let stream = open(url, path, key).await;
    let mut reader = BufReader::new(stream);
    let mut out = Vec::new();
    for _ in 0..lines {
        let mut line = String::new();
        match tokio::time::timeout(PATIENCE, reader.read_line(&mut line)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(_)) => out.push(line.trim_end().to_owned()),
            _ => break,
        }
    }
    out
}

/// The SSE request itself, as a live stream, so a case can hold it open.
pub async fn open(url: &str, path: &str, key: &str) -> TcpStream {
    let rest = url.trim_start_matches("http://");
    let mut stream = TcpStream::connect(rest).await.expect("the hub's port");
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {rest}\r\nAccept: text/event-stream\r\n\
         Authorization: Bearer {key}\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .expect("the request head");
    stream
}

/// A short read budget, so a case waiting for a line the hub will not send fails instead of hanging.
pub const PATIENCE: std::time::Duration = std::time::Duration::from_secs(10);

/// The database URL every case's hub reads, or a panic naming the command that sets it.
pub fn url() -> String {
    db::url()
}
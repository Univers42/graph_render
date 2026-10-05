//! What the SSE cases share: a hub over a real socket, a chunked-body reader, and the seams.
//!
//! The socket is the point, and so is the chunk decoder. An SSE stream has no end a test can wait
//! for, so a case reads a bounded prefix of the bytes and hangs up; §5.3's claims are claims about
//! those bytes — that `busy` carries no `id:` line, that a heartbeat carries no `data:` — and none
//! of them is visible through a typed event. The body arrives chunked, so the reader decodes it: a
//! raw read would see a hex length where a notice is.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;

use graph_store::StoreError;

use crate::support::fixtures::{hub_db, ready, upsert};
use crate::support::{Hub, db};

/// The name of the key every fixture mints, which is what a subscriber slot is counted under.
///
/// The slot is counted by key **name**, never by the secret: the hub only ever learns the name, so
/// a case that counted by the token would find nothing.
pub const KEY: &str = crate::support::KEY_NAME;

/// A short read budget, so a case waiting for a line the hub will not send fails instead of hanging.
pub const PATIENCE: std::time::Duration = std::time::Duration::from_secs(10);

/// A TCP listener on loopback, and the URL of a hub serving `router` on it.
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

/// The SSE request as a live stream, so a case can hold it open while another arrives.
pub async fn open(url: &str, path: &str, key: &str) -> tokio::net::TcpStream {
    use tokio::io::AsyncWriteExt;
    let host = url.trim_start_matches("http://").to_owned();
    let mut stream = tokio::net::TcpStream::connect(&host)
        .await
        .expect("the hub's port");
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}\r\nAccept: text/event-stream\r\n\
         Authorization: Bearer {key}\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .await
        .expect("the request head");
    stream
}

/// One SSE request's status line and its body's lines, with the chunked framing decoded away.
///
/// The status line comes first so a case can assert a 429 without reading a single event, and the
/// body lines are what §5.3's wire claims are about.
pub async fn raw_events(url: &str, path: &str, key: &str, lines: usize) -> Vec<String> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let stream = open(url, path, key).await;
    let mut reader = BufReader::new(stream);
    let mut out = Vec::new();
    read_line(&mut reader, &mut out).await;
    loop {
        let mut header = String::new();
        match tokio::time::timeout(PATIENCE, reader.read_line(&mut header)).await {
            Ok(Ok(0)) => return out,
            Ok(Ok(_)) if header.trim().is_empty() => break,
            Ok(Ok(_)) => out.push(header.trim_end().to_owned()),
            _ => return out,
        }
    }
    if std::env::var("GM_DEBUG_RAW").is_ok() {
        use tokio::io::AsyncReadExt;
        let mut raw = Vec::new();
        let _ = tokio::time::timeout(
            std::time::Duration::from_millis(1500),
            reader.read_to_end(&mut raw),
        )
        .await;
        eprintln!("RAWALL {:?}", String::from_utf8_lossy(&raw));
        return out;
    }
    read_body(&mut reader, &mut out, lines).await;
    out
}

/// Read `count` more body lines, through the chunked framing.
async fn read_body<R>(reader: &mut R, out: &mut Vec<String>, count: usize)
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    use tokio::io::{AsyncBufReadExt, AsyncReadExt};
    let mut taken = 0;
    while taken < count {
        let mut size_line = String::new();
        match tokio::time::timeout(PATIENCE, reader.read_line(&mut size_line)).await {
            Ok(Ok(0)) => return,
            Ok(Ok(_)) => {}
            _ => return,
        }
        // The chunk length is hex, per RFC 9112 §7.1: "f" is fifteen bytes, and reading it as
        // decimal is the mistake that makes every SSE body look empty.
        let Ok(size) = usize::from_str_radix(size_line.trim(), 16) else {
            return;
        };
        if size == 0 {
            return;
        }
        let mut chunk = vec![0u8; size];
        if tokio::time::timeout(PATIENCE, reader.read_exact(&mut chunk))
            .await
            .is_err()
        {
            return;
        }
        let mut trailer = [0u8; 2];
        let _ = reader.read_exact(&mut trailer).await;
        if std::env::var("GM_DEBUG_RAW").is_ok() {
            eprintln!("RAW line={:?} size={size} bytes={:?}", size_line, String::from_utf8_lossy(&chunk));
        }
        for line in String::from_utf8_lossy(&chunk).lines() {
            out.push(line.to_owned());
            taken += 1;
        }
    }
}

/// Read one line into `out`, and stop there.
async fn read_line<R>(reader: &mut R, out: &mut Vec<String>)
where
    R: tokio::io::AsyncBufRead + Unpin,
{
    use tokio::io::AsyncBufReadExt;
    let mut line = String::new();
    if let Ok(Ok(_)) = tokio::time::timeout(PATIENCE, reader.read_line(&mut line)).await {
        out.push(line.trim_end().to_owned());
    }
}

/// Make every stream read meet `fault`.
///
/// The seam lives in the `App` behind an `Arc`, which is why installing it needs no `&mut`: the slot
/// is interior-mutable (`hooks::Slot`), so a fixture can install a fault after the router exists.
pub fn fault_on_reads(app: &Arc<graph_hub::App>, fault: fn() -> StoreError) {
    let boxed: graph_hub::hooks::FaultHook = Arc::new(move |_since| Some(fault()));
    install(&app.hooks.page_fault, boxed);
}

/// Stop faulting stream reads, so the next read is real.
pub fn read_for_real(app: &Arc<graph_hub::App>) {
    clear(&app.hooks.page_fault);
}

/// Count every request that admitted a permit, which is how a case proves a reconnect made none.
///
/// Caveat: the counter is on `pause_after_admit`, which every route calls *after* its permit and
/// *before* its work, so a non-zero count means "some route ran", not "some `/graph` ran". The case
/// that uses it asserts zero, which is the direction where the distinction does not matter.
pub fn count_admitted(app: &Arc<graph_hub::App>) -> Arc<AtomicU64> {
    let seen = Arc::new(AtomicU64::new(0));
    let counter = Arc::clone(&seen);
    let hook: graph_hub::hooks::AsyncHook = Arc::new(move || {
        let counter = Arc::clone(&counter);
        Box::pin(async move {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        })
    });
    install(&app.hooks.pause_after_admit, hook);
    seen
}

/// A hub with `ws` created, one plugin registered and one committed change, over `env`.
pub async fn hub_with_one_change(env: &[(&str, &str)], prefix: &str) -> (Hub, String) {
    let hub = hub_db(env).await;
    let ws = db::unique(prefix);
    ready(&hub, &ws, "task").await;
    hub.post(
        &format!("/v1/workspaces/{ws}/plugins/task/batches"),
        upsert("task", "one", "n"),
    )
    .await;
    (hub, ws)
}

/// The workspace's epoch, read through the listing rather than guessed by a fixture.
pub async fn epoch_of(hub: &Hub, ws: &str) -> String {
    let reply = hub.get_with("/v1/workspaces").await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON list");
    value["workspaces"]
        .as_array()
        .expect("a workspaces array")
        .iter()
        .find(|row| row["id"] == ws)
        .and_then(|row| row["epoch"].as_u64())
        .unwrap_or_else(|| panic!("the listing holds {ws}"))
        .to_string()
}

/// Wait until `key` holds a subscriber slot, so a case's writes land after the stream is connected.
///
/// The alternative is a sleep, and a sleep is a race that passes on a fast host and fails on a slow
/// one; the count is the fact the case actually needs.
pub async fn until_subscribed(hub: &Hub) {
    for _ in 0..200 {
        if hub.app.gates.subscribers.of(KEY) > 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("the stream never took a subscriber slot");
}

/// Write `hook` into `slot`, replacing whatever was there.
#[cfg(feature = "test-hooks")]
fn install<T>(slot: &graph_hub::hooks::Slot<T>, hook: T) {
    let mut held = slot.lock().expect("the seam slot is not poisoned");
    *held = Some(hook);
}

/// Empty `slot`, so the call sites fold away again.
#[cfg(feature = "test-hooks")]
fn clear<T>(slot: &graph_hub::hooks::Slot<T>) {
    let mut held = slot.lock().expect("the seam slot is not poisoned");
    *held = None;
}

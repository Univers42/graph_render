//! The body reader's four refusals and the three connection limits hyper enforces before a request
//! reaches the router.
//!
//! The body cases drive `body::read` directly, because a route that reads a body is Task 5's; what
//! is under test here is the reader, and the reader is the only thing between a caller's upload and
//! the hub's memory.
//!
//! The connection cases need a real listener and a real socket, because hyper's header read timeout,
//! its read buffer and the accept loop's connection cap are all properties of the connection and not
//! of a request. So these three bind `127.0.0.1:0`, speak HTTP/1.1 over a `TcpStream`, and read the
//! answer off the wire.

use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::HeaderMap;
use axum::response::IntoResponse;
use graph_hub::{body, serve};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::support::*;

/// The budget every body case uses, short enough that a 408 is a 408 and not a hang.
const PATIENCE: Duration = Duration::from_millis(200);

/// How long a connection case waits for an answer that is supposed to arrive.
const REACH: Duration = Duration::from_secs(5);

/// A declared `Content-Length` over the cap is a 413 **before a byte is read**: the reader never
/// touches the body, which is what makes a 5 GiB upload cost one header comparison.
#[tokio::test]
async fn a_body_over_the_cap_is_413_without_reading_a_byte() {
    let (stream, read) = counting_body();
    let mut headers = HeaderMap::new();
    headers.insert("content-length", (5u64 << 30).to_string().parse().unwrap());
    let refused = body::read(stream, &headers, 1024, PATIENCE)
        .await
        .unwrap_err();
    assert_eq!(refused.status(), 413);
    assert_eq!(refused.code(), "too_large");
    assert_eq!(read(), 0, "no byte was pulled from the body");
}

/// A chunked body over the cap is a 413 mid-stream, which is the only place a length limit can catch
/// it: there is no declared length to compare.
#[tokio::test]
async fn a_chunked_body_over_the_cap_is_413_mid_stream() {
    let stream = Body::from_stream(futures_util::stream::iter(vec![
        Ok::<_, std::io::Error>(bytes("x".repeat(2048))),
        Ok(bytes("y".repeat(2048))),
    ]));
    let refused = body::read(stream, &HeaderMap::new(), 1024, PATIENCE)
        .await
        .unwrap_err();
    assert_eq!(refused.status(), 413, "{}", refused.code());
}

/// A body that stalls past `GRAPH_HUB_BODY_TIMEOUT_MS` is a 408: the hub's own budget, which
/// graph-server enforces independently on its side of the relay.
#[tokio::test]
async fn a_body_that_stalls_past_the_body_timeout_is_408() {
    let stream = Body::from_stream(futures_util::stream::once(async {
        tokio::time::sleep(Duration::from_secs(30)).await;
        Ok::<_, std::io::Error>(bytes(String::from("late")))
    }));
    let refused = body::read(stream, &HeaderMap::new(), 1 << 20, PATIENCE)
        .await
        .unwrap_err();
    assert_eq!(refused.status(), 408);
    assert_eq!(refused.code(), "Timeout");
}

/// A body under the cap is returned whole, and an empty one is not a refusal.
#[tokio::test]
async fn a_body_under_the_cap_is_read_whole() {
    for payload in ["".to_owned(), "{}".to_owned(), "x".repeat(1024)] {
        let read = body::read(
            Body::from(payload.clone()),
            &HeaderMap::new(),
            1024,
            PATIENCE,
        )
        .await
        .unwrap_or_else(|error| panic!("{payload:?}: {error:?}"));
        assert_eq!(read.len(), payload.len(), "{payload:?}");
    }
}

/// A body that cannot be read at all is a 400, not a 413: a client that reset mid-body did not send
/// something too large, it sent something unusable.
#[tokio::test]
async fn a_body_that_cannot_be_read_is_400() {
    let failing = Body::from_stream(futures_util::stream::once(async {
        Err::<Bytes, std::io::Error>(std::io::Error::other("reset"))
    }));
    let refused = body::read(failing, &HeaderMap::new(), 1 << 20, PATIENCE)
        .await
        .unwrap_err();
    assert_eq!(refused.status(), 400);
    assert_eq!(refused.code(), "BadRequest");
}

/// A `Content-Length` that does not parse is not a 413: the hub only refuses a number it read, and
/// hyper refuses such a request before it reaches the reader anyway.
#[tokio::test]
async fn a_declared_length_that_does_not_parse_is_not_a_413() {
    let mut headers = HeaderMap::new();
    headers.insert("content-length", "not-a-number".parse().unwrap());
    let read = body::read(Body::from("{}"), &headers, 1024, PATIENCE)
        .await
        .unwrap_or_else(|error| panic!("{error:?}"));
    assert_eq!(read.len(), 2);
}

/// A 413 names the limit it was over, so a caller can see which knob to move.
#[tokio::test]
async fn a_413_names_its_limit() {
    let mut headers = HeaderMap::new();
    headers.insert("content-length", "4096".parse().unwrap());
    let refused = body::read(Body::from("{}"), &headers, 1024, PATIENCE)
        .await
        .unwrap_err();
    let body = read_body(refused.into_response()).await;
    assert!(body.contains("1024"), "the body names the limit: {body}");
}

/// `GRAPH_HUB_MAX_HEADER_BYTES`: a head past it is hyper's bare 431, not the JSON error body, and it
/// never reaches a handler — so it is read off the wire rather than from `oneshot`.
#[tokio::test]
async fn a_header_over_the_cap_is_431() {
    let hub = hub_with_env(&[("GRAPH_HUB_MAX_HEADER_BYTES", "8192")]);
    let addr = serving(&hub).await;
    let padding = "x".repeat(16 * 1024);
    let request = format!("GET /healthz HTTP/1.1\r\nHost: x\r\nX-Pad: {padding}\r\n\r\n");
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream
        .write_all(request.as_bytes())
        .await
        .expect("write the head");
    let answer = read_answer(&mut stream, Duration::from_millis(500)).await;
    assert!(
        answer.starts_with("HTTP/1.1 431"),
        "a head past GRAPH_HUB_MAX_HEADER_BYTES is a bare 431: {answer:?}"
    );
}

/// `GRAPH_HUB_HEADER_TIMEOUT_MS`: a head that arrives slowly is cut, and the connection closes with
/// no answer at all.
#[tokio::test]
async fn a_header_read_past_the_header_timeout_closes_the_connection() {
    let hub = hub_with_env(&[("GRAPH_HUB_HEADER_TIMEOUT_MS", "150")]);
    let addr = serving(&hub).await;
    let mut stream = TcpStream::connect(addr).await.expect("connect");
    stream
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\n")
        .await
        .expect("a partial head");
    let answer = read_answer(&mut stream, Duration::from_millis(600)).await;
    assert!(
        answer.is_empty(),
        "the connection is closed with no answer: {answer:?}"
    );
}

/// `GRAPH_HUB_MAX_CONNECTIONS`: the permit is held from before `accept`, so past the cap a connection
/// waits in the kernel backlog and is served once one frees. It is not refused.
#[tokio::test]
async fn the_connection_cap_queues_the_next_connection() {
    let hub = hub_with_env(&[("GRAPH_HUB_MAX_CONNECTIONS", "1")]);
    let addr = serving(&hub).await;
    // The first connection holds the only slot with a head that never finishes.
    let mut held = TcpStream::connect(addr)
        .await
        .expect("the first connection");
    held.write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\n")
        .await
        .expect("a partial head");
    tokio::time::sleep(Duration::from_millis(100)).await;
    // The second is queued rather than served, and rather than refused. `try_read` is what proves
    // that without consuming anything: a read here would eat the answer the case then waits for.
    let mut queued = TcpStream::connect(addr)
        .await
        .expect("the second connection");
    queued
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: x\r\nConnection: close\r\n\r\n")
        .await
        .expect("a whole head");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        matches!(queued.try_read(&mut [0u8; 1]), Err(error) if error.kind() == std::io::ErrorKind::WouldBlock),
        "the second connection is queued, not served and not refused"
    );
    drop(held);
    // Once the slot frees, the queued connection is served: the hub waited rather than refused.
    let answer = read_answer(&mut queued, REACH).await;
    assert!(
        answer.starts_with("HTTP/1.1 200"),
        "the queued connection is served once a slot frees: {answer:?}"
    );
}

/// A hub under these settings serves `/healthz` on a loopback listener, and the address it bound.
async fn serving(hub: &Hub) -> std::net::SocketAddr {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .expect("the listener binds");
    let addr = listener.local_addr().expect("the listener address");
    let app = Arc::clone(&hub.app);
    let connections = hub.app.settings.connections;
    tokio::spawn(async move {
        let _ = serve::serve_on(listener, &connections, app).await;
    });
    addr
}

/// Read whatever `stream` answers within `budget`, as text. An empty answer is a closed connection.
async fn read_answer(stream: &mut TcpStream, budget: Duration) -> String {
    let mut answer = Vec::new();
    let _ = tokio::time::timeout(budget, stream.read_to_end(&mut answer)).await;
    String::from_utf8_lossy(&answer).into_owned()
}

/// Read a response body as text.
async fn read_body(response: axum::response::Response) -> String {
    use http_body_util::BodyExt;
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("the body")
        .to_bytes();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Bytes for the streamed bodies above.
fn bytes(text: String) -> Bytes {
    Bytes::from(text)
}

/// A body stream that counts how many frames were pulled from it, so "without reading a byte" is an
/// observation and not an assertion about the code's shape.
fn counting_body() -> (Body, impl Fn() -> usize) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let pulled = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&pulled);
    let stream = futures_util::stream::unfold(0u8, move |state| {
        let counter = Arc::clone(&counter);
        async move {
            if state == 0 {
                Some((Ok::<_, std::io::Error>(Bytes::new()), 1))
            } else {
                counter.fetch_add(1, Ordering::SeqCst);
                None
            }
        }
    });
    (Body::from_stream(stream), move || {
        pulled.load(Ordering::SeqCst)
    })
}

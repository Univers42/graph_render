//! Verdict condition 12, row `svc-shutdown`, on a real process: `SIGTERM` stops new connections at
//! once, lets the request already in flight finish with its normal status, and the process exits 0
//! within `GRAPH_TIMEOUT_MS`.

mod common;

use common::{Child, PATIENCE, bearer, doc, head, setup, status_of};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// The drain budget this server is given, the same figure `serve::drain` spends.
const DRAIN_MS: &str = "8000";
/// How long the listener is given to stop accepting. Caveat: a fixed bound, so a host slower than
/// this reads as "still serving", not as a hang.
const REFUSAL_WINDOW: Duration = Duration::from_secs(2);
/// The pause between the head landing and `SIGTERM`, so the request is in flight. Caveat: a guess,
/// not a measurement: long enough for the server to accept and start reading the body, short
/// enough to sit far inside the drain budget.
const IN_FLIGHT: Duration = Duration::from_millis(250);

/// The graph the in-flight request carries: small enough to fit the body cap and to run well
/// inside the drain budget, large enough that the run is real work and not an empty reply.
const GRAPH: (usize, usize) = (64, 96);

#[test]
fn a_sigterm_drains_the_request_in_flight_and_exits_0() {
    let mut child = setup(&[("GRAPH_TIMEOUT_MS", DRAIN_MS)]).spawn();
    let body = doc(GRAPH.0, GRAPH.1);
    let (status, answer) = drain_the_request(&mut child, body.as_bytes());
    assert_eq!(
        status, 200,
        "the request in flight did not finish: {answer}"
    );
    let stopped = child.wait_line(|line| line["event"] == "stopped");
    assert_eq!(stopped["drained"], true, "{stopped}");
    let status = child
        .exit_within(drain_budget())
        .unwrap_or_else(|| panic!("the process outlived its drain budget of {DRAIN_MS} ms"));
    assert_eq!(status.code(), Some(0), "exit status {status}");
}

#[test]
fn a_new_connection_after_sigterm_is_refused() {
    let mut child = setup(&[]).spawn();
    child.signal("TERM");
    let status = child
        .exit_within(drain_budget())
        .unwrap_or_else(|| panic!("the process outlived its drain budget of {DRAIN_MS} ms"));
    assert_eq!(status.code(), Some(0), "exit status {status}");
    assert!(
        refused(child.addr),
        "a new connection to {} was served after SIGTERM",
        child.addr
    );
}

/// Opens a keyed `POST /v1/layout`, sends `SIGTERM` while the body is half through, and returns the
/// answer the request finishes with. The head and half the body are written, the listener is
/// checked to have stopped accepting, then the rest of the body goes out and the answer is read.
fn drain_the_request(child: &mut Child, body: &[u8]) -> (u16, String) {
    let half = body.len() / 2;
    let mut stream = open_mid_body(child, body, half);
    std::thread::sleep(IN_FLIGHT);
    child.signal("TERM");
    assert!(
        refused_within(child.addr, REFUSAL_WINDOW),
        "a new connection to {} was served after SIGTERM",
        child.addr
    );
    stream
        .write_all(&body[half..])
        .expect("the rest of the body");
    read_answer(stream)
}

/// A keyed `POST /v1/layout` whose head and first `sent` body bytes are written: the request is
/// accepted and its body is still arriving.
fn open_mid_body(child: &Child, body: &[u8], sent: usize) -> TcpStream {
    let length = format!("Content-Length: {}", body.len());
    let key = bearer(&child.key);
    let head = head("POST", "/v1/layout?layout=layout.grid", &[length, key]);
    let mut stream = TcpStream::connect(child.addr).expect("connect");
    stream.set_read_timeout(Some(PATIENCE)).expect("timeout");
    stream.write_all(head.as_bytes()).expect("the head");
    stream
        .write_all(&body[..sent])
        .expect("the first of the body");
    stream
}

/// The whole answer, read to the close: its status and its text.
fn read_answer(mut stream: TcpStream) -> (u16, String) {
    let mut answer = Vec::new();
    let _read = stream.read_to_end(&mut answer);
    let text = String::from_utf8_lossy(&answer).into_owned();
    (status_of(&text), text)
}

/// True once `addr` refuses a connection.
fn refused(addr: SocketAddr) -> bool {
    TcpStream::connect(addr).is_err()
}

/// [`refused`], polled until it holds or `window` passes.
fn refused_within(addr: SocketAddr, window: Duration) -> bool {
    let deadline = Instant::now() + window;
    while Instant::now() < deadline {
        if refused(addr) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// The drain budget [`drain_the_request`]'s server was given.
fn drain_budget() -> Duration {
    Duration::from_millis(DRAIN_MS.parse().expect("a millisecond figure"))
}

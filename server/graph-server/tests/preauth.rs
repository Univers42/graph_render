//! Verdict condition 4, row `svc-preauth`, on a real process: a keyless body is refused before a
//! byte of it is buffered, and a request head sent slowly is cut at `GRAPH_HEADER_TIMEOUT_MS`. The
//! last two cases are the connection limits of the same condition, which had no enforcement test
//! (docs/reviews/review-svc-r3.md finding, low): `GRAPH_MAX_CONNECTIONS` is held from before
//! `accept`, and `GRAPH_MAX_HEADER_BYTES` is hyper's read buffer.

mod common;

use common::{PATIENCE, exchange, head, setup, status_of};
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

const BODY_BYTES: usize = 64 << 20;
const BODIES: usize = 50;
/// Bodies in flight at once. Caveat: 25, not 50, so the negative control (every body read,
/// about twice its size each while collected) peaks near 3.2 GiB and stays under the 8 GiB
/// container cap; a 137 there would hide which test failed.
const IN_FLIGHT: usize = 25;
const RSS_BUDGET_KB: u64 = 16 << 10;

#[test]
fn fifty_keyless_full_bodies_are_401_and_hold_no_memory() {
    let child = setup(&[]).spawn();
    assert_eq!(send_keyless(child.addr, &Arc::new(vec![b' '; 16])), 401);
    let baseline = child.vm_kb("VmRSS");
    child.reset_peak();
    let (body, addr) = (Arc::new(vec![b' '; BODY_BYTES]), child.addr);
    for _wave in 0..BODIES / IN_FLIGHT {
        let senders: Vec<_> = (0..IN_FLIGHT)
            .map(|_| {
                let body = Arc::clone(&body);
                std::thread::spawn(move || send_keyless(addr, &body))
            })
            .collect();
        for sender in senders {
            assert_eq!(sender.join().expect("a sender"), 401);
        }
    }
    let grown = child.vm_kb("VmHWM").saturating_sub(baseline);
    assert!(
        grown < RSS_BUDGET_KB,
        "the peak RSS grew {grown} kB over a {baseline} kB baseline (budget {RSS_BUDGET_KB} kB)"
    );
}

/// `POST /v1/layout` with no key and `body`, written while the answer is read on another thread:
/// the server answers 401 before the body and closes, so the write may fail with a reset.
fn send_keyless(addr: SocketAddr, body: &Arc<Vec<u8>>) -> u16 {
    let length = format!("Content-Length: {}", body.len());
    let head = head("POST", "/v1/layout?layout=layout.grid", &[length]);
    let mut stream = TcpStream::connect(addr).expect("connect");
    let reader = stream.try_clone().expect("a second handle");
    let answer = std::thread::spawn(move || read_status(reader));
    let _refused = stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body));
    answer.join().expect("the reader")
}

/// The status line's code, read until the line ends, the peer closes or [`PATIENCE`] passes.
fn read_status(mut stream: TcpStream) -> u16 {
    stream.set_read_timeout(Some(PATIENCE)).expect("timeout");
    let mut text = Vec::new();
    let mut chunk = [0u8; 512];
    while !text.windows(2).any(|pair| pair == b"\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(read) => text.extend_from_slice(&chunk[..read]),
        }
    }
    status_of(&String::from_utf8_lossy(&text))
}

#[test]
fn headers_at_one_byte_a_second_are_cut_at_the_timeout() {
    let child = setup(&[("GRAPH_HEADER_TIMEOUT_MS", "2000")]).spawn();
    let head = head("GET", "/healthz", &[]);
    let mut stream = TcpStream::connect(child.addr).expect("connect");
    stream
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("timeout");
    let start = Instant::now();
    let cut = head.bytes().take(10).find_map(|byte| {
        if stream.write_all(&[byte]).is_err() {
            return Some(start.elapsed());
        }
        let mut answer = [0u8; 256];
        match stream.read(&mut answer) {
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => None,
            _closed_or_answered => Some(start.elapsed()),
        }
    });
    let cut = cut.expect("a head sent at 1 byte/s was still open after 10 s");
    let window = Duration::from_millis(1500)..Duration::from_secs(5);
    assert!(
        window.contains(&cut),
        "cut after {cut:?}, not near the 2 s timeout"
    );
}

#[test]
fn a_connection_past_graph_max_connections_is_not_accepted_until_a_slot_is_free() {
    let child = setup(&[("GRAPH_MAX_CONNECTIONS", "1")]).spawn();
    // The first connection takes the one permit: it is answered and then held open, with no
    // `Connection: close`, so the permit is still held when the second one arrives.
    let mut first = TcpStream::connect(child.addr).expect("connect");
    first.set_read_timeout(Some(PATIENCE)).expect("timeout");
    first
        .write_all(kept_open("/healthz").as_bytes())
        .expect("the first request");
    let mut answered = [0u8; 256];
    let read = first.read(&mut answered).expect("the first answer");
    assert_eq!(
        status_of(&String::from_utf8_lossy(&answered[..read])),
        200,
        "the first connection holds the only permit"
    );
    // The second connection waits in the kernel backlog: no permit, no `accept`.
    // Caveat: 500 ms is a guess at a served answer's arrival, and the read must fail rather than
    // block for PATIENCE, so a served connection reads as an error and fails the assert.
    let mut second = TcpStream::connect(child.addr).expect("connect");
    second
        .set_read_timeout(Some(Duration::from_millis(500)))
        .expect("timeout");
    second
        .write_all(head("GET", "/healthz", &[]).as_bytes())
        .expect("the second request");
    let mut premature = [0u8; 256];
    assert!(
        second.read(&mut premature).is_err(),
        "a second connection was served past GRAPH_MAX_CONNECTIONS=1"
    );
    // Dropping the first releases its permit, and the second is accepted and answered.
    drop(first);
    second.set_read_timeout(Some(PATIENCE)).expect("timeout");
    let mut answer = Vec::new();
    second.read_to_end(&mut answer).expect("the second answer");
    assert_eq!(status_of(&String::from_utf8_lossy(&answer)), 200);
}

#[test]
fn a_head_over_graph_max_header_bytes_is_431() {
    let child = setup(&[("GRAPH_MAX_HEADER_BYTES", "8192")]).spawn();
    // 16 KiB of padding, past hyper's read buffer and past the 8 KiB floor the range allows.
    let padding = "x".repeat(16 << 10);
    let request = head("GET", "/healthz", &[format!("X-Padding: {padding}")]);
    let (status, answer) = exchange(child.addr, request.as_bytes());
    assert_eq!(status, 431, "hyper answered {answer}");
}

/// A request head with no `Connection: close`, so the connection outlives the answer.
fn kept_open(target: &str) -> String {
    format!("GET {target} HTTP/1.1\r\nHost: test\r\n\r\n")
}

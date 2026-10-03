//! Verdict condition 11: `graph-server healthcheck`, the image's `HEALTHCHECK`, exits 0 on a live
//! server and 1 on a closed port or a listener that never answers, within its 2 s budget.

mod common;

use common::setup;
use std::net::TcpListener;
use std::process::Command;
use std::time::{Duration, Instant};

/// `graph-server healthcheck` against `port`: its exit code and how long it took.
fn healthcheck(port: u16) -> (Option<i32>, Duration) {
    let start = Instant::now();
    let status = Command::new(env!("CARGO_BIN_EXE_graph-server"))
        .arg("healthcheck")
        .env_clear()
        .env("GRAPH_PORT", port.to_string())
        .status()
        .expect("healthcheck runs");
    (status.code(), start.elapsed())
}

#[test]
fn a_live_server_is_healthy() {
    let child = setup(&[]).spawn();
    assert_eq!(healthcheck(child.addr.port()).0, Some(0));
}

#[test]
fn a_closed_port_is_unhealthy() {
    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port();
    assert_eq!(healthcheck(port).0, Some(1));
}

#[test]
fn a_silent_listener_is_unhealthy_within_the_budget() {
    let silent = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = silent.local_addr().expect("addr").port();
    let (code, took) = healthcheck(port);
    assert_eq!(code, Some(1));
    assert!(took < Duration::from_secs(3), "took {took:?}");
    drop(silent);
}

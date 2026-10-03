//! The limits (Verdict conditions 4 and 5), rows `svc-preauth` and `svc-limits` in process:
//! the body cap holds for a declared length and for a chunked body, a slow body is cut, a full
//! queue is a 429, a timed-out run keeps its slot until it ends, and a panic is a 500 that the
//! process survives.

mod common;

use axum::body::Body;
use axum::http::StatusCode;
use common::{Hold, chunked, doc, server, server_with, until};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[tokio::test]
async fn a_declared_length_over_the_limit_is_413_unread() {
    let server = server(&[("GRAPH_MAX_BODY", "1024")]);
    let request = server.request("POST", "/v1/layout?layout=layout.grid");
    let request = request
        .header("content-length", "65536")
        .body(common::failing_body());
    let reply = server.send(request.unwrap()).await;
    assert_eq!(
        (reply.status, reply.code().as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, "IngestTooLarge")
    );
}

#[tokio::test]
async fn a_chunked_body_over_the_limit_is_413() {
    let server = server(&[("GRAPH_MAX_BODY", "1024")]);
    let reply = server
        .layout("layout=layout.grid", chunked(512, 3, Duration::ZERO))
        .await;
    assert_eq!(
        (reply.status, reply.code().as_str()),
        (StatusCode::PAYLOAD_TOO_LARGE, "IngestTooLarge")
    );
    let reply = server.layout("layout=layout.grid", doc(3, 2)).await;
    assert_eq!(
        reply.status,
        StatusCode::OK,
        "a body under the limit still runs"
    );
}

#[tokio::test]
async fn a_slow_body_is_408() {
    let server = server(&[("GRAPH_BODY_TIMEOUT_MS", "100")]);
    let reply = server
        .layout(
            "layout=layout.grid",
            chunked(8, 5, Duration::from_millis(300)),
        )
        .await;
    assert_eq!(
        (reply.status, reply.code().as_str()),
        (StatusCode::REQUEST_TIMEOUT, "Timeout")
    );
}

#[tokio::test]
async fn a_full_queue_is_429_with_retry_after() {
    let hold = Hold::new();
    let _release = hold.release_on_drop();
    let server = server_with(
        &[("GRAPH_WORKERS", "1"), ("GRAPH_QUEUE", "1")],
        hold.hooks(),
    );
    let running = server.spawn(layout_request(&server));
    hold.reached(1).await;
    let queued = server.spawn(layout_request(&server));
    tokio::time::sleep(Duration::from_millis(50)).await;
    let reply = server.send(layout_request(&server)).await;
    assert_eq!(
        (reply.status, reply.code().as_str()),
        (StatusCode::TOO_MANY_REQUESTS, "Busy")
    );
    assert_eq!(reply.header("retry-after"), "1");
    hold.release();
    assert_eq!(running.await.unwrap().status, StatusCode::OK);
    assert_eq!(queued.await.unwrap().status, StatusCode::OK);
}

#[tokio::test]
async fn a_timed_out_run_keeps_its_slot_until_it_ends() {
    let hold = Hold::new();
    let _release = hold.release_on_drop();
    let env = [
        ("GRAPH_WORKERS", "2"),
        ("GRAPH_QUEUE", "0"),
        ("GRAPH_TIMEOUT_MS", "200"),
    ];
    let server = server_with(&env, hold.hooks());
    let first = [
        server.spawn(layout_request(&server)),
        server.spawn(layout_request(&server)),
    ];
    for timed_out in first {
        let reply = timed_out.await.unwrap();
        assert_eq!(
            (reply.status, reply.code().as_str()),
            (StatusCode::SERVICE_UNAVAILABLE, "Timeout")
        );
    }
    // workers + queue more requests, while both runs still hold their slots.
    for _ in 0..2 {
        let reply = server.send(layout_request(&server)).await;
        assert_eq!(
            (reply.status, reply.code().as_str()),
            (StatusCode::TOO_MANY_REQUESTS, "Busy")
        );
    }
    hold.release();
    until(|| server.app.gate.free() == 2).await;
    let reply = server.send(layout_request(&server)).await;
    assert_eq!(reply.status, StatusCode::OK);
}

#[tokio::test]
async fn a_panic_is_500_and_the_next_run_is_200() {
    let armed = Arc::new(AtomicBool::new(true));
    let fault = Arc::clone(&armed);
    let hook = move || assert!(!fault.swap(false, Ordering::SeqCst), "the test fault");
    let hooks = graph_server::app::Hooks {
        before_run: Some(Arc::new(hook)),
    };
    let server = server_with(&[("GRAPH_WORKERS", "1")], hooks);
    let reply = server.send(layout_request(&server)).await;
    assert_eq!(
        (reply.status, reply.code().as_str()),
        (StatusCode::INTERNAL_SERVER_ERROR, "Internal")
    );
    let reply = server.send(layout_request(&server)).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(server.app.gate.free(), 1);
}

fn layout_request(server: &common::Server) -> axum::http::Request<Body> {
    let request = server.request("POST", "/v1/layout?layout=layout.grid");
    request.body(Body::from(doc(5, 4))).unwrap()
}

//! The roundtrip itself: the hub's answer is graph-server's own bytes, and `drop-record` is in the
//! relay and nowhere else.

use std::sync::Arc;

use axum::body::Body;
use graph_hub::relay::body::Probe;
use support::Reply;

use super::*;

/// §8's `hub-roundtrip`: over each of its three fixtures, the hub's answer is byte-identical to what
/// graph-server answers for `/graph`'s own document at the same cursor.
#[tokio::test]
async fn layout_bytes_equal_motor_bytes_at_the_same_cursor() {
    let (motor, key) = real_with_key().await;
    let hub = hub_against(&motor, &key, &[]).await;
    for ws in FIXTURES {
        fixture(&hub, ws).await;
        let hub_side = lay_out(&hub, ws).await;
        assert_eq!(hub_side.code(), 200, "{ws}: {}", hub_side.body());
        let read = graph(&hub, ws).await;
        assert_eq!(read.code(), 200, "{ws}: {}", read.body());
        let motor_side = at_the_motor(&motor, &key, &read.body()).await;
        assert_eq!(motor_side.code(), 200, "{ws}: {}", motor_side.body());
        assert_eq!(
            hub_side.body.len(),
            motor_side.body.len(),
            "{ws}: the two answers are different lengths"
        );
        assert!(
            hub_side.body == motor_side.body,
            "{ws}: the hub's bytes are not graph-server's bytes"
        );
    }
}

/// One fixture of §8's three: `plain` is a single plugin with no links, `linked` adds a reference
/// that resolves across plugins, and `pruned` names a record nobody stored.
async fn fixture(hub: &Hub, ws: &str) {
    match ws {
        "relay-plain" => single_plugin(hub, ws, 6).await,
        _ => {
            single_plugin(hub, ws, 4).await;
            second_plugin(hub, ws).await;
            memo(
                hub,
                ws,
                if ws == "relay-linked" {
                    "id0000"
                } else {
                    "nowhere"
                },
            )
            .await;
        }
    }
}

/// `POST /v1/layout?layout=…&source=contract` at graph-server itself, with `doc` as the body.
///
/// `source=contract` is the same one the relay sends: H1 says the hub asks for the contract layout
/// because the document it streams *is* the ingest contract document.
async fn at_the_motor(motor: &Motor, key: &str, doc: &str) -> Reply {
    let uri = format!(
        "{}/v1/layout?layout={LAYOUT}&source=contract",
        motor.url("")
    );
    use hyper_util::client::legacy::Client;
    use hyper_util::client::legacy::connect::HttpConnector;
    use hyper_util::rt::TokioExecutor;
    let client: Client<HttpConnector, Body> = Client::builder(TokioExecutor::new()).build_http();
    let request = hyper::Request::builder()
        .method("POST")
        .uri(uri)
        .header("authorization", format!("Bearer {key}"))
        .body(Body::from(doc.to_owned()))
        .expect("the motor request");
    let response = client.request(request).await.expect("a motor answer");
    let (parts, body) = response.into_parts();
    Reply::from_parts(parts, body).await
}

/// `/graph` is **not** the relay's stream: `drop-record` drops one record in `/layout` and nowhere
/// else, so the two routes cannot change together (§5.3, D4).
///
/// This is the half of row `negctl-drop-record` that must stay green: the control runs the byte
/// equality and requires it to fail, and runs this and requires it to pass. Without it the control
/// would only prove that some relay exists, not that the break is in the relay alone.
#[tokio::test]
async fn graph_is_not_affected_by_drop_record() {
    let (motor, key) = real_with_key().await;
    let hub = hub_against(&motor, &key, &[]).await;
    let count = 6;
    single_plugin(&hub, "relay-whole", count).await;
    let read = graph(&hub, "relay-whole").await;
    assert_eq!(read.code(), 200, "{}", read.body());
    for i in 0..count {
        assert!(
            read.body().contains(&format!("id{i:04}")),
            "record id{i:04} is missing from /graph"
        );
    }
    let (body, dropped) = streamed_by_the_relay(&hub, "relay-whole").await;
    assert_eq!(
        dropped,
        u64::from(graph_hub::breaks::on("drop-record")),
        "the relay drops exactly one record, and only with the break on"
    );
    if dropped == 1 {
        assert_ne!(
            body, read.body,
            "with drop-record on the two routes must differ by one record"
        );
    } else {
        assert_eq!(
            body, read.body,
            "with no break the relay streams the same bytes /graph serves"
        );
    }
}

/// What `relay::body::document` streamed for `ws`: the request body the relay sends to the motor,
/// and how many records `drop-record` skipped in it.
///
/// Caveat: it drives the walk directly rather than reading the socket, because the bytes the motor
/// received are not observable from here and its **answer** is a different document. What the claim is
/// about is the request body, and this is the request body.
async fn streamed_by_the_relay(hub: &Hub, ws: &str) -> (Vec<u8>, u64) {
    use futures_util::StreamExt;
    let store = hub.app.store().await.expect("the store under test");
    let document = graph_store::materialize::open(store, ws)
        .await
        .expect("a snapshot of the workspace");
    let probe = Arc::new(Probe::default());
    let deadline = tokio::time::Instant::now() + hub.app.settings.limits.stream_deadline;
    let mut walk = Box::pin(graph_hub::relay::body::document(
        document,
        Some(Arc::clone(&probe)),
        deadline,
    ));
    let mut out: Vec<u8> = Vec::new();
    while let Some(chunk) = walk.next().await {
        out.extend_from_slice(&chunk.expect("a document chunk"));
    }
    (out, probe.counts().dropped)
}

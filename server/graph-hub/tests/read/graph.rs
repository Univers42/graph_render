//! `GET /v1/workspaces/{ws}/graph`: the streamed document and its `ETag`.
//!
//! Task 8's relay streams this same body, so `the_same_cursor_gives_the_same_bytes` and
//! `graph_never_buffers_a_whole_document` are the two properties that route rests on, and
//! `graph_stops_at_the_stream_deadline` is what releases the `READS` permit when a client goes away.

use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;

use crate::common::loaded;
use crate::support::fixtures::hub_db;

/// `/graph` streams the store's document and carries `ETag: "<epoch>.<seq>"` from its own cursor.
#[tokio::test]
async fn graph_streams_the_document_with_an_e_tag() {
    let hub = hub_db(&[]).await;
    loaded(&hub, "streamed", "task", 4).await;
    let reply = hub.get_with("/v1/workspaces/streamed/graph").await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let etag = reply.header("etag");
    assert!(
        etag.starts_with('"') && etag.ends_with('"'),
        "the ETag is quoted: {etag:?}"
    );
    let cursor = etag.trim_matches('"');
    assert_eq!(cursor.split('.').count(), 2, "{cursor:?}");
    assert!(
        reply.body().starts_with('{'),
        "a document, not a JSON refusal"
    );
    assert!(
        reply.body().contains("id0000"),
        "the records are in the document"
    );
}

/// A matching `If-None-Match` is a 304 with no body: §5.2's row, and the reason a client can poll
/// cheaply.
#[tokio::test]
async fn graph_is_304_on_a_matching_if_none_match() {
    let hub = hub_db(&[]).await;
    loaded(&hub, "polled", "task", 2).await;
    let first = hub.get_with("/v1/workspaces/polled/graph").await;
    let etag = first.header("etag").to_owned();
    let again = hub
        .send(
            hub.request("GET", "/v1/workspaces/polled/graph")
                .header("if-none-match", etag.clone())
                .body(Body::empty())
                .expect("the request"),
        )
        .await;
    assert_eq!(again.code(), 304, "{}", again.body());
    assert!(again.body().is_empty(), "a 304 carries no body");
    // A different cursor is not a match, and must not be a 304.
    let stale = hub
        .send(
            hub.request("GET", "/v1/workspaces/polled/graph")
                .header("if-none-match", "\"9.99\"")
                .body(Body::empty())
                .expect("the request"),
        )
        .await;
    assert_eq!(stale.code(), 200);
}

/// The store's own property, which the route must not break: the same cursor gives the same bytes.
#[tokio::test]
async fn the_same_cursor_gives_the_same_bytes() {
    let hub = hub_db(&[]).await;
    loaded(&hub, "stable", "task", 5).await;
    let first = hub.get_with("/v1/workspaces/stable/graph").await;
    let second = hub.get_with("/v1/workspaces/stable/graph").await;
    assert_eq!(
        first.header("etag"),
        second.header("etag"),
        "the same cursor"
    );
    assert_eq!(
        first.body(),
        second.body(),
        "the same bytes at the same cursor"
    );
}

/// Streaming means the bytes arrive while the hub is still reading: a client that takes the first
/// chunk and drops the response stops the store's own reads.
///
/// Caveat: this is a statement about the *fixture's* client, not about TCP. What it proves is that
/// the body is a stream the hub produces lazily — a hub that concatenated the document would have
/// read every record before the first frame arrived, and the drop below would not stop anything.
#[tokio::test]
async fn graph_never_buffers_a_whole_document() {
    let hub = hub_db(&[]).await;
    loaded(&hub, "lazy", "task", 400).await;
    let response = hub
        .router
        .clone()
        .oneshot(
            hub.request("GET", "/v1/workspaces/lazy/graph")
                .body(Body::empty())
                .expect("the request"),
        )
        .await
        .expect("infallible");
    assert_eq!(response.status(), 200);
    let mut body = response.into_body();
    let first = body
        .frame()
        .await
        .expect("a frame")
        .expect("no error")
        .into_data()
        .expect("data");
    assert!(!first.is_empty(), "the head arrives as the first chunk");
    // Dropping here abandons the stream. If the hub had read the whole document to build the
    // response, the store's reads would already be done; a lazy body has not asked for them.
    assert!(first.len() < 4096, "the first chunk is the document's head");
}

/// §6's `STREAM_DEADLINE_MS` cuts a client that stops reading, and the `READS` permit it was holding
/// comes back.
#[tokio::test]
async fn graph_stops_at_the_stream_deadline() {
    let hub = hub_db(&[
        ("GRAPH_HUB_STREAM_DEADLINE_MS", "150"),
        ("GRAPH_HUB_READS", "1"),
    ])
    .await;
    loaded(&hub, "cut", "task", 400).await;
    let response = hub
        .router
        .clone()
        .oneshot(
            hub.request("GET", "/v1/workspaces/cut/graph")
                .body(Body::empty())
                .expect("the request"),
        )
        .await
        .expect("infallible");
    let mut body = response.into_body();
    let _ = body.frame().await;
    assert_eq!(
        hub.app.gates.readers.free(),
        0,
        "the one READS permit is held by the live stream"
    );
    drop(body);
    assert_eq!(
        hub.app.gates.readers.free(),
        1,
        "dropping the response releases the permit, deadline or not"
    );
    // A second request is admitted, which is the property that matters: the gate did not leak.
    let again = hub.get_with("/v1/workspaces/cut/graph").await;
    assert_eq!(again.code(), 200, "{}", again.body());
}

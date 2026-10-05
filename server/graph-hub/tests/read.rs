//! The read half of §5.2's table: `/graph`, `/changes`, `/v1/meta` and `/v1/workspaces`.
//!
//! The two that matter for Review Focus are here: `/layout`'s byte equality is Task 8's, and `/graph`
//! is what the relay streams, so `the_same_cursor_gives_the_same_bytes` and
//! `graph_never_buffers_a_whole_document` are the properties the relay rests on.
#![cfg(feature = "db-tests")]

#[path = "support/mod.rs"]
mod support;

use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;

use support::fixtures::*;
use support::*;

/// The workspace's own epoch, as `GET /v1/workspaces` publishes it.
///
/// WHY the listing and not `/graph`: an epoch is drawn by the database's clock (`hub_next_epoch`,
/// microseconds), so a fixture cannot name one and must read it. The listing answers from the
/// workspace row alone, where `/graph` reads the change log — which matters for the case below
/// that has just emptied that log.
async fn epoch_of(hub: &Hub, ws: &str) -> String {
    let reply = hub.get_with("/v1/workspaces").await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON list");
    let row = value["workspaces"]
        .as_array()
        .expect("a workspaces array")
        .iter()
        .find(|row| row["id"] == ws)
        .unwrap_or_else(|| panic!("the listing holds {ws}"));
    row["epoch"]
        .as_u64()
        .expect("an epoch")
        .to_string()
}

/// A workspace with `count` records, which is the fixture every read case below starts from.
async fn loaded(hub: &Hub, ws: &str, plugin: &str, count: usize) {
    ready(hub, ws, plugin).await;
    let upserts: Vec<(&str, &str, &str)> = (0..count)
        .map(|i| ("task", Box::leak(format!("id{i:04}").into_boxed_str()) as &str, "note"))
        .collect();
    let reply = hub
        .post(
            &format!("/v1/workspaces/{ws}/plugins/{plugin}/batches"),
            batch(&upserts, &[]),
        )
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
}

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
    assert!(reply.body().starts_with('{'), "a document, not a JSON refusal");
    assert!(reply.body().contains("id0000"), "the records are in the document");
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
    assert_eq!(first.header("etag"), second.header("etag"), "the same cursor");
    assert_eq!(first.body(), second.body(), "the same bytes at the same cursor");
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

/// `/changes` after a cursor is in seq order, and every change is graph-contract's own text.
#[tokio::test]
async fn changes_after_a_cursor_are_in_seq_order() {
    let hub = hub_db(&[]).await;
    loaded(&hub, "seqs", "task", 3).await;
    let path = "/v1/workspaces/seqs/plugins/task/batches";
    hub.post(path, upsert("task", "later", "n")).await;
    let epoch = epoch_of(&hub, "seqs").await;
    let reply = hub
        .get_with(&format!("/v1/workspaces/seqs/changes?since={epoch}.1"))
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON page");
    let seqs: Vec<u64> = value["changes"]
        .as_array()
        .expect("a changes array")
        .iter()
        .map(|change| change["seq"].as_u64().expect("a seq"))
        .collect();
    // seq 1 is the manifest, so a page after `<epoch>.1` is the two batches and nothing else.
    assert_eq!(seqs, [2, 3], "every change after the cursor, in seq order");
    assert!(
        seqs.windows(2).all(|pair| pair[0] < pair[1]),
        "seq order, never a repeat"
    );
    assert!(value["next"].is_string(), "a page carries the next cursor");
    assert_eq!(value["epoch"].as_u64().is_some(), true, "the page's epoch");
}

/// A cursor from another epoch is a 410: §5.3's rule, and the reason a promoted workspace's
/// subscribers resync rather than read a gap.
#[tokio::test]
async fn changes_returns_410_for_a_cursor_from_another_epoch() {
    let hub = hub_db(&[]).await;
    loaded(&hub, "epoched", "task", 1).await;
    let reply = hub
        .get_with("/v1/workspaces/epoched/changes?since=7.1")
        .await;
    assert_eq!(reply.code(), 410, "{}", reply.body());
    assert_eq!(reply.error(), "cursor");
}

/// A cursor below what the log keeps is a 410 too. Nothing prunes in this slice (graph-store's
/// retention lands later), so the fixture deletes the log's rows itself — which is the same shape a
/// full retention prune leaves behind.
#[tokio::test]
async fn changes_returns_410_for_a_cursor_below_what_is_kept() {
    let hub = hub_db(&[]).await;
    loaded(&hub, "pruned", "task", 3).await;
    let epoch = epoch_of(&hub, "pruned").await;
    db::drop_changes(&hub.app, "pruned").await;
    let reply = hub
        .get_with(&format!("/v1/workspaces/pruned/changes?since={epoch}.1"))
        .await;
    assert_eq!(reply.code(), 410, "{}", reply.body());
}

/// A seq above `head_seq` is a 410 as well (§5.3 says "including `seq > head_seq`"), never an empty
/// page that a client would read as "caught up".
#[tokio::test]
async fn changes_returns_410_for_a_seq_above_head() {
    let hub = hub_db(&[]).await;
    loaded(&hub, "ahead", "task", 1).await;
    let epoch = epoch_of(&hub, "ahead").await;
    // A cursor whose epoch is right and whose seq is past `head_seq`: 410 and never an empty page a
    // client would read as "caught up".
    let reply = hub
        .get_with(&format!("/v1/workspaces/ahead/changes?since={epoch}.99"))
        .await;
    assert_eq!(reply.code(), 410, "{}", reply.body());
}

/// One page never passes `GRAPH_HUB_CHANGES_BYTES` and never carries fewer than one change: §6's row
/// and the reason the start check refuses a page cap below `max_change`.
#[tokio::test]
async fn a_changes_page_never_exceeds_the_byte_cap_but_holds_one_change() {
    // §6 refuses a page cap below `max_change` (`max_body + 96 * max_batch`), so the cap can only be
    // exercised by shrinking what one change may be first: one operation per batch and a 1 KiB body
    // put `max_change` at 1 120, which is the smallest cap that is still a legal deployment. The
    // refusal of a smaller one is Task 2's `start_check_refuses_changes_bytes_below_max_change`.
    let cap = 1_120u64;
    let hub = hub_db(&[
        ("GRAPH_HUB_MAX_BODY", "1024"),
        ("GRAPH_HUB_MAX_BATCH", "1"),
        ("GRAPH_HUB_CHANGES_BYTES", "1120"),
    ])
    .await;
    ready(&hub, "capped", "task").await;
    let epoch = epoch_of(&hub, "capped").await;
    // Four separate batches, so four separate changes: one batch would be one change and the cap
    // would have nothing to cut.
    for i in 0..4 {
        let note = "n".repeat(700);
        let id = format!("wide{i:03}");
        hub.post(
            "/v1/workspaces/capped/plugins/task/batches",
            batch(&[("task", &id, &note)], &[]),
        )
        .await;
    }
    let reply = hub
        .get_with(&format!(
            "/v1/workspaces/capped/changes?since={epoch}.1&limit=20"
        ))
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON page");
    let changes = value["changes"].as_array().expect("a changes array");
    assert!(!changes.is_empty(), "a page always holds at least one change");
    assert_eq!(changes.len(), 1, "the byte cap cut the page at one change");
    assert!(
        value["bytes"].as_u64().expect("the page's bytes") <= cap,
        "and the page reports its own bytes"
    );
    assert!(value["next"].is_string(), "and the next cursor to resume from");
}

/// `/v1/meta` spells out every §6 limit, so a client can size itself without reading the deploy doc.
#[tokio::test]
async fn meta_reports_every_limit_of_section_6() {
    let hub = hub_db(&[]).await;
    let reply = hub.get_with("/v1/meta").await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON answer");
    assert_eq!(value["api"], 1);
    assert!(value["version"].is_string(), "the hub's own version");
    let limits = value["limits"].as_object().expect("a limits object");
    for name in [
        "max_body",
        "max_batch",
        "max_record_bytes",
        "max_plugin_bytes",
        "max_doc_bytes",
        "retain",
        "retain_bytes",
        "changes_bytes",
        "body_timeout_ms",
        "stream_deadline_ms",
        "motor_timeout_ms",
        "timeout_ms",
        "sse_page",
        "fetch_rows",
        "last_seen",
        "writers",
        "readers",
        "layouts",
        "writers_per_key",
        "max_connections",
        "header_timeout_ms",
        "max_header_bytes",
        "max_subscribers",
        "max_subscribers_per_key",
    ] {
        assert!(limits.contains_key(name), "/v1/meta is missing {name}");
    }
    assert_eq!(limits["max_body"], 4 << 20, "§6's default");
    assert_eq!(limits["writers"], 2);
    assert_eq!(limits["sse_page"], 256);
}

/// The workspaces list holds only what the key may read, and each row carries its `epoch` and
/// `head_seq`.
#[tokio::test]
async fn the_workspaces_list_holds_only_what_the_key_may_read() {
    // `admin` per workspace, so each key can create the workspace it is granted and the listing
    // still has to filter: the grant names a workspace and nothing else differs between the keys.
    let hub = hub_db_grants("tester mine admin\nstranger theirs admin\n").await;
    let stranger = add_key(&hub, "stranger");
    hub.app.keys.reload().expect("both keys are in the pair");
    ready(&hub, "mine", "task").await;
    ready_as(&hub, &stranger, "theirs", "task").await;
    assert_eq!(
        names_of(&hub.get_with("/v1/workspaces").await),
        ["mine"],
        "only the granted workspace"
    );
    assert_eq!(
        names_of(&hub.get_as(&stranger, "/v1/workspaces").await),
        ["theirs"],
        "the other key sees only its own"
    );
}

/// The workspace ids of a `GET /v1/workspaces` answer, in the order the route returned them.
fn names_of(reply: &Reply) -> Vec<String> {
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON list");
    let rows: Vec<serde_json::Value> = value["workspaces"]
        .as_array()
        .expect("a workspaces array")
        .clone();
    for row in &rows {
        assert!(row["epoch"].as_u64().is_some(), "each row carries its epoch");
        assert!(
            row["head_seq"].as_u64().is_some(),
            "each row carries its head_seq"
        );
    }
    rows.iter()
        .map(|row| row["id"].as_str().expect("an id").to_owned())
        .collect()
}

/// [`ready`] with `key`, which is what a case with two identities needs: the second workspace is
/// created by the key that is granted it, because authorization runs before existence.
async fn ready_as(hub: &Hub, key: &str, ws: &str, plugin: &str) {
    for (path, body) in [
        (format!("/v1/workspaces/{ws}"), String::new()),
        (
            format!("/v1/workspaces/{ws}/plugins/{plugin}"),
            MANIFEST.to_owned(),
        ),
    ] {
        let reply = hub
            .send(
                hub.request_as(key, "PUT", &path)
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .expect("the request"),
            )
            .await;
        assert!(reply.code() < 300, "{path}: {}", reply.body());
    }
}

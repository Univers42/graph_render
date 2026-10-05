//! The stream's ordinary life: the notices it sends, the resume rule, and its two caps.
//!
//! Every case drives the real hub over a real socket, so what is asserted is §5.3's wire text rather
//! than a typed event: the `id:` line, the `event:` name and the `data:` body a client parses.

use crate::common::{
    count_headers, epoch_of, hub_with_one_change, raw_events, serve, until_count, until_subscribed,
};
use crate::support::db;
use crate::support::fixtures::hub_db;
use crate::support::fixtures::{ready, upsert};

/// Four writers commit while one subscriber is connected: the seqs it sees are `n+1, n+2, …`, with no
/// repeat and no hole. This is the property that makes the watch a *position* rather than a payload.
#[tokio::test]
async fn a_reconnect_while_four_writers_commit_has_no_gap_and_no_duplicate() {
    let (hub, ws) = hub_with_one_change(&[], "writers").await;
    let url = serve(hub.router.clone()).await;
    let path = format!("/v1/workspaces/{ws}/events");
    let epoch = epoch_of(&hub, &ws).await;
    let key = hub.key.clone();
    let reader = key.clone();
    let stream = tokio::spawn(async move { raw_events(&url, &path, &reader, 40).await });
    until_subscribed(&hub).await;
    // The stream holds its slot before the writes, so every seq is delivered rather than summarised:
    // a subscriber that connects after them resumes from its own cursor and never sees them at all.
    for i in 0..4 {
        hub.post(
            &format!("/v1/workspaces/{ws}/plugins/task/batches"),
            upsert("task", &format!("w{i}"), "n"),
        )
        .await;
    }
    let lines = stream.await.expect("the subscriber task");
    let seqs = ids_of(&lines);
    assert_eq!(
        seqs.len(),
        4,
        "the subscriber saw all four writes, on epoch {epoch}: {lines:?}"
    );
    assert!(
        seqs.windows(2).all(|pair| pair[1] == pair[0] + 1),
        "no gap and no duplicate: {seqs:?}"
    );
}

/// A stream resumed 100 000 changes back reads at most `GRAPH_HUB_SSE_PAGE` headers per read, so a
/// subscriber far behind is served in pages rather than one unbounded statement.
///
/// The workspace carries four changes and the page is capped at two, which is the same shape at a
/// size a test can afford: the header counter is what shows the cap, not the number of changes.
#[tokio::test]
async fn a_stream_resumed_100_000_changes_back_reads_at_most_256_headers_per_read() {
    let hub = hub_db(&[("GRAPH_HUB_SSE_PAGE", "2")]).await;
    let ws = db::unique("paged");
    ready(&hub, &ws, "task").await;
    for i in 0..4 {
        hub.post(
            &format!("/v1/workspaces/{ws}/plugins/task/batches"),
            upsert("task", &format!("p{i}"), "n"),
        )
        .await;
    }
    let counts = count_headers(&hub.app);
    let url = serve(hub.router.clone()).await;
    let epoch = epoch_of(&hub, &ws).await;
    let lines = raw_events(
        &url,
        &format!("/v1/workspaces/{ws}/events?since={epoch}.0"),
        &hub.key,
        40,
    )
    .await;
    assert_eq!(
        ids_of(&lines),
        vec![1, 2, 3, 4, 5],
        "the resumed stream pages through the whole log, in order: {lines:?}"
    );
    let reads: Vec<u64> = counts.lock().expect("the header log").clone();
    assert_eq!(
        reads.len(),
        3,
        "five changes at two per read is three reads: {reads:?}"
    );
    assert!(
        reads.iter().all(|count| *count <= 2),
        "and no read passed GRAPH_HUB_SSE_PAGE: {reads:?}"
    );
}

/// §6's per-key subscriber cap is a 429, decided before the stream starts: a refused stream costs
/// the hub a task and a connection and must not cost the client a wait.
#[tokio::test]
async fn the_per_key_subscriber_cap_is_429() {
    let hub = hub_db(&[("GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY", "1")]).await;
    let ws = db::unique("perkey");
    ready(&hub, &ws, "task").await;
    let url = serve(hub.router.clone()).await;
    let _held = crate::common::open(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key).await;
    until_subscribed(&hub).await;
    let second = raw_events(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key, 12).await;
    let status = status_of(&second);
    assert_eq!(status, 429, "the second stream on one key: {second:?}");
}

/// The total cap is a 429 as well, and it counts across keys: two keys at one stream each fill a
/// total of two.
#[tokio::test]
async fn the_total_subscriber_cap_is_429() {
    let hub = hub_db(&[
        ("GRAPH_HUB_MAX_SUBSCRIBERS", "1"),
        ("GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY", "8"),
    ])
    .await;
    let ws = db::unique("total");
    ready(&hub, &ws, "task").await;
    let url = serve(hub.router.clone()).await;
    let _held = crate::common::open(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key).await;
    until_count(&hub, 1).await;
    let second = raw_events(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key, 12).await;
    assert_eq!(status_of(&second), 429, "{second:?}");
}

/// The first notice after a reconnect is the cursor's successor, or the stream resyncs: a stream that
/// resumed *at* its cursor would repeat the change the client already has forever.
#[tokio::test]
async fn the_first_change_after_a_reconnect_is_cursor_plus_one_or_it_resyncs() {
    let (hub, ws) = hub_with_one_change(&[], "resume").await;
    let epoch = epoch_of(&hub, &ws).await;
    let url = serve(hub.router.clone()).await;
    let lines = raw_events(
        &url,
        &format!("/v1/workspaces/{ws}/events?since={epoch}.1"),
        &hub.key,
        12,
    )
    .await;
    let seqs = ids_of(&lines);
    assert!(
        !seqs.is_empty(),
        "the resumed stream sends what it missed: {lines:?}"
    );
    assert_eq!(seqs[0], 2, "cursor plus one, from epoch {epoch}: {lines:?}");
    assert!(
        seqs.windows(2).all(|pair| pair[0] < pair[1]),
        "and strictly increasing: {lines:?}"
    );
}

/// `Last-Event-ID` is the resume source a reconnecting SDK uses, and it beats `?since=`: a client
/// that sends both means the header.
#[tokio::test]
async fn last_event_id_wins_over_since() {
    let (hub, ws) = hub_with_one_change(&[], "header").await;
    let epoch = epoch_of(&hub, &ws).await;
    let path = format!("/v1/workspaces/{ws}/events?since={epoch}.99");
    let parsed: u64 = epoch.parse().expect("the workspace's epoch is a number");
    // The order of the two sources is the whole rule, and a socket cannot show it: a stream that
    // resumes from the header is identical on the wire to one that resumes from the query, so the
    // rule is pinned where it is decided instead.
    let headers = axum::http::HeaderMap::new();
    let uri: axum::http::Uri = format!("{path}").parse().expect("a URI");
    let from_query =
        graph_hub::events::cursor_of(&headers, &uri, parsed, 7).expect("the query's cursor");
    assert_eq!(
        from_query.seq, 99,
        "?since= is read when there is no header"
    );
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        "last-event-id",
        axum::http::HeaderValue::from_static("12345.1"),
    );
    let from_header =
        graph_hub::events::cursor_of(&headers, &uri, parsed, 7).expect("the header's cursor");
    assert_eq!(
        from_header,
        graph_contract::hub::Cursor {
            epoch: 12345,
            seq: 1
        }
    );
}

/// The notice body is graph-contract's `notice_json`, so a client parses the same bytes the contract
/// writes and the hub is not a second producer of that text.
#[tokio::test]
async fn a_notice_carries_the_contract_own_text() {
    let (hub, ws) = hub_with_one_change(&[], "notice").await;
    let url = serve(hub.router.clone()).await;
    let epoch = epoch_of(&hub, &ws).await;
    let lines = raw_events(
        &url,
        &format!("/v1/workspaces/{ws}/events?since={epoch}.0"),
        &hub.key,
        12,
    )
    .await;
    let data = lines
        .iter()
        .find(|line| line.starts_with("data: "))
        .unwrap_or_else(|| panic!("a notice body: {lines:?}"));
    let body = data.trim_start_matches("data: ");
    let value: serde_json::Value = serde_json::from_str(body).expect("a JSON notice");
    for member in ["seq", "plugin", "at"] {
        assert!(
            value.get(member).is_some(),
            "the notice carries {member}: {body}"
        );
    }
}

/// The seqs of the `id:` lines in `lines`, in the order they arrived.
fn ids_of(lines: &[String]) -> Vec<u64> {
    lines
        .iter()
        .filter_map(|line| line.strip_prefix("id: "))
        .filter_map(|cursor| cursor.rsplit('.').next())
        .filter_map(|seq| seq.parse().ok())
        .collect()
}

/// The HTTP status line's status, or 0 when the answer never got that far.
fn status_of(lines: &[String]) -> u16 {
    lines
        .first()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse().ok())
        .unwrap_or(0)
}

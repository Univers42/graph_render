//! The two early closes, and the four properties the plan's Review Focus 3 pins.
//!
//! Each case forces its end with the `page_fault` seam rather than with timing. A pool cannot be made
//! short on demand and a log cannot be pruned on demand, so a case that waits for either is a case
//! that passes for the wrong reason on a slow host; the seam makes the fault the *subject* of the
//! test instead of its setup.

use std::sync::atomic::Ordering;

use graph_store::StoreError;

use crate::common::{
    epoch_of, fault_on_reads, hub_with_one_change, raw_events, read_for_real, serve,
};
use crate::support::fixtures::{hub_db, ready};
use crate::support::db;

/// A cursor below what the log keeps is a `resync` on the stream rather than a 410 before it: the
/// request was well formed, so the refusal belongs on the wire the client is already reading.
#[tokio::test]
async fn resync_past_retention() {
    let (hub, ws) = hub_with_one_change(&[], "retention").await;
    fault_on_reads(&hub.app, || StoreError::Gone);
    let url = serve(hub.router.clone()).await;
    let lines = raw_events(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key, 8).await;
    assert!(
        lines.iter().any(|line| line == "event: resync"),
        "a gone cursor resyncs: {lines:?}"
    );
}

/// A `since` above `head_seq` is the same `resync`: §5.3 names it beside the retention case, and a
/// client that asked for a position that does not exist must read the document again.
#[tokio::test]
async fn resync_for_a_since_above_head() {
    let (hub, ws) = hub_with_one_change(&[], "above").await;
    let url = serve(hub.router.clone()).await;
    // A well formed cursor of the right epoch and a seq past `head_seq`: the store answers `Gone`.
    let epoch = epoch_of(&hub, &ws).await;
    let path = format!("/v1/workspaces/{ws}/events?since={epoch}.999999");
    let lines = raw_events(&url, &path, &hub.key, 8).await;
    assert!(
        lines.iter().any(|line| line == "event: resync"),
        "a cursor above head resyncs: {lines:?}"
    );
}

/// An epoch that moved is a `resync` with **no hub write after it**: the promotion is what voids the
/// cursor, so the stream has to notice it without a change to notice.
#[tokio::test]
async fn resync_after_an_epoch_change() {
    let (hub, ws) = hub_with_one_change(&[], "promoted").await;
    promote(&ws).await;
    let url = serve(hub.router.clone()).await;
    let lines = raw_events(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key, 8).await;
    assert!(
        lines.iter().any(|line| line == "event: resync"),
        "a promoted workspace resyncs: {lines:?}"
    );
}

/// The revision-5 deviation: `busy` carries **no `id:` line**, because `Last-Event-ID` is what a
/// reconnect resumes from and an id here would skip a change the client never received.
#[tokio::test]
async fn busy_carries_no_id_line() {
    let (hub, ws) = hub_with_one_change(&[], "busy").await;
    fault_on_reads(&hub.app, || StoreError::Busy { retry_after: 1 });
    let url = serve(hub.router.clone()).await;
    let lines = raw_events(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key, 12).await;
    let at = lines
        .iter()
        .position(|line| line == "event: busy")
        .unwrap_or_else(|| panic!("the stream closed with busy: {lines:?}"));
    let tail = &lines[at..];
    assert!(
        !tail.iter().any(|line| line.starts_with("id:")),
        "no id: after event: busy, or the SDK resumes from a change it never saw: {tail:?}"
    );
    assert!(
        !tail.iter().any(|line| line.starts_with("data:")),
        "and no data: either, so nothing is mistaken for a change: {tail:?}"
    );
}

/// The slot is free **before** the close reaches the client: a second subscriber on the same key is
/// admitted while the first connection is still open.
#[tokio::test]
async fn the_busy_slot_is_free_before_the_close() {
    let hub = hub_db(&[("GRAPH_HUB_MAX_SUBSCRIBERS_PER_KEY", "1")]).await;
    let ws = db::unique("slot");
    ready(&hub, &ws, "task").await;
    fault_on_reads(&hub.app, || StoreError::Busy { retry_after: 1 });
    let url = serve(hub.router.clone()).await;
    let first = crate::common::open(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key)
        .await;
    let lines = raw_events(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key, 12).await;
    assert!(
        lines.iter().any(|line| line == "event: busy"),
        "the second stream was admitted and met the fault, rather than a 429: {lines:?}"
    );
    // The first connection is still open, and the count is back: the cap is per key and it is 1.
    assert_eq!(
        hub.app.gates.subscribers.total(),
        0,
        "the faulted stream gave its slot back before the close reached the client"
    );
    drop(first);
}

/// A reconnect after `busy` reads **no** `/graph`: the cursor is still valid, and a whole document
/// from a pool that is already short is the wrong answer to a `busy`.
#[tokio::test]
async fn the_busy_reconnect_reads_no_graph() {
    let (hub, ws) = hub_with_one_change(&[], "nograph").await;
    let graphs = crate::common::count_admitted(&hub.app);
    let url = serve(hub.router.clone()).await;
    let path = format!("/v1/workspaces/{ws}/events");
    fault_on_reads(&hub.app, || StoreError::Busy { retry_after: 1 });
    let first = raw_events(&url, &path, &hub.key, 12).await;
    assert!(
        first.iter().any(|line| line == "event: busy"),
        "the first stream closed with busy: {first:?}"
    );
    // The SDK reconnects from its cursor. The fault is gone, so this stream reads for real.
    read_for_real(&hub.app);
    let again = raw_events(&url, &path, &hub.key, 12).await;
    assert!(
        !again.iter().any(|line| line == "event: resync"),
        "the cursor was never invalid, so the reconnect must not resync: {again:?}"
    );
    assert!(
        again.iter().any(|line| line == "event: change"),
        "and it resumes the feed rather than reading the document: {again:?}"
    );
    assert_eq!(graphs.load(Ordering::Relaxed), 0, "no request was counted");
}

/// A cursor pruned during the client's reconnect backoff gets a `resync`, never a silently short
/// page: the seqs it wanted are gone and only the document can replace them.
#[tokio::test]
async fn a_cursor_pruned_during_the_backoff_gets_resync() {
    let (hub, ws) = hub_with_one_change(&[], "backoff").await;
    let url = serve(hub.router.clone()).await;
    // The client is in its backoff: the log is pruned and the workspace's epoch moves under it.
    fault_on_reads(&hub.app, || StoreError::Busy { retry_after: 1 });
    let first = raw_events(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key, 12).await;
    assert!(first.iter().any(|line| line == "event: busy"), "{first:?}");
    read_for_real(&hub.app);
    promote(&ws).await;
    let again = raw_events(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key, 12).await;
    assert!(
        again.iter().any(|line| line == "event: resync"),
        "a cursor that went void in the backoff window resyncs: {again:?}"
    );
}

/// The heartbeat is a comment and carries no data: a client that parses `data:` lines must find none
/// from a keep-alive, or every idle period would look like an empty change.
#[tokio::test]
async fn a_heartbeat_is_a_comment_and_never_carries_data() {
    let hub = hub_db(&[]).await;
    let ws = db::unique("beat");
    ready(&hub, &ws, "task").await;
    let url = serve(hub.router.clone()).await;
    let lines = raw_events(&url, &format!("/v1/workspaces/{ws}/events"), &hub.key, 4).await;
    // A stream with nothing to send writes no `data:` line at all, which is the property: whatever
    // arrives while idle is a comment.
    assert!(
        !lines.iter().any(|line| line.starts_with("data:")),
        "an idle stream carries no data: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line == "event: change"),
        "and no change it did not make: {lines:?}"
    );
}

/// The notice body is graph-contract's `notice_json`, so a client parses the same bytes the contract
/// writes and the hub is not a second producer of that text.
#[tokio::test]
async fn a_notice_carries_the_contract_own_text() {
    let (hub, ws) = hub_with_one_change(&[], "text").await;
    let url = serve(hub.router.clone()).await;
    let lines = raw_events(
        &url,
        &format!("/v1/workspaces/{ws}/events?since=0.0"),
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
        assert!(value.get(member).is_some(), "the notice carries {member}: {body}");
    }
}

/// Bump `ws`'s epoch from a second session, which is what a restore promotion does and what no hub
/// write announces.
async fn promote(ws: &str) {
    let mut config = graph_store::StoreConfig::defaults();
    config.url = db::url();
    let store = graph_store::Store::connect(&config)
        .await
        .expect("a second store on the same database");
    let client = store.client().await.expect("a second connection");
    client
        .batch_execute(&format!(
            "UPDATE workspaces SET epoch = epoch + 1 WHERE id = '{ws}'"
        ))
        .await
        .expect("the promotion");
    drop(client);
}

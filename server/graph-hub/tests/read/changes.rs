//! `GET /v1/workspaces/{ws}/changes`: one bounded page of the feed after a cursor.
//!
//! Every refusal here is a cursor refusal, so the four `410` cases differ only in which bound of
//! §5.3 the cursor falls outside, and the byte cap case is the one that shows a page is bounded and
//! never empty.

use crate::common::{epoch_of, loaded};
use crate::support::db;
use crate::support::fixtures::{batch, hub_db, ready, upsert};

/// `/changes` after a cursor is in seq order, and every change is graph-contract's own text.
#[tokio::test]
async fn changes_after_a_cursor_are_in_seq_order() {
    let hub = hub_db(&[]).await;
    let ws = db::unique("seqs");
    loaded(&hub, &ws, "task", 3).await;
    let path = format!("/v1/workspaces/{ws}/plugins/task/batches");
    hub.post(&path, upsert("task", "later", "n")).await;
    let epoch = epoch_of(&hub, &ws).await;
    let reply = hub
        .get_with(&format!("/v1/workspaces/{ws}/changes?since={epoch}.1"))
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
    assert!(
        value["epoch"].as_u64().is_some(),
        "the page carries its epoch"
    );
}

/// A cursor from another epoch is a 410: §5.3's rule, and the reason a promoted workspace's
/// subscribers resync rather than read a gap.
#[tokio::test]
async fn changes_returns_410_for_a_cursor_from_another_epoch() {
    let hub = hub_db(&[]).await;
    let ws = db::unique("epoched");
    loaded(&hub, &ws, "task", 1).await;
    let reply = hub
        .get_with(&format!("/v1/workspaces/{ws}/changes?since=7.1"))
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
    // A name no earlier run used, and emptied first: this case deletes the workspace's manifest
    // change, and `put_manifest` writes no seq for byte-identical content, so a reused name could
    // never register its plugin again and the case would fail in the fixture rather than here.
    let ws = db::unique("pruned");
    loaded(&hub, &ws, "task", 3).await;
    let epoch = epoch_of(&hub, &ws).await;
    db::drop_changes(&hub.app, &ws).await;
    let reply = hub
        .get_with(&format!("/v1/workspaces/{ws}/changes?since={epoch}.1"))
        .await;
    assert_eq!(reply.code(), 410, "{}", reply.body());
}

/// A seq above `head_seq` is a 410 as well (§5.3 says "including `seq > head_seq`"), never an empty
/// page that a client would read as "caught up".
#[tokio::test]
async fn changes_returns_410_for_a_seq_above_head() {
    let hub = hub_db(&[]).await;
    let ws = db::unique("ahead");
    loaded(&hub, &ws, "task", 1).await;
    let epoch = epoch_of(&hub, &ws).await;
    // A cursor whose epoch is right and whose seq is past `head_seq`: 410 and never an empty page a
    // client would read as "caught up".
    let reply = hub
        .get_with(&format!("/v1/workspaces/{ws}/changes?since={epoch}.99"))
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
    let ws = db::unique("capped");
    ready(&hub, &ws, "task").await;
    let epoch = epoch_of(&hub, &ws).await;
    // Four separate batches, so four separate changes: one batch would be one change and the cap
    // would have nothing to cut.
    for i in 0..4 {
        let note = "n".repeat(700);
        let id = format!("wide{i:03}");
        hub.post(
            &format!("/v1/workspaces/{ws}/plugins/task/batches"),
            batch(&[("task", &id, &note)], &[]),
        )
        .await;
    }
    let reply = hub
        .get_with(&format!(
            "/v1/workspaces/{ws}/changes?since={epoch}.1&limit=20"
        ))
        .await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON page");
    let changes = value["changes"].as_array().expect("a changes array");
    assert!(
        !changes.is_empty(),
        "a page always holds at least one change"
    );
    assert_eq!(changes.len(), 1, "the byte cap cut the page at one change");
    assert!(
        value["bytes"].as_u64().expect("the page's bytes") <= cap,
        "and the page reports its own bytes"
    );
    assert!(
        value["next"].is_string(),
        "and the next cursor to resume from"
    );
}

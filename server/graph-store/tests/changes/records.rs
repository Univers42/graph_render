//! The records page: byte order, `plugin_seq`, and a `next` that ends.

use super::*;
use graph_store::records::{RecordsPage, RecordsReq, page};

/// The records request for `tracker`.
fn records(after: Option<(String, String)>, limit: u64) -> RecordsReq {
    RecordsReq {
        ws: "ws".to_owned(),
        plugin: "tracker".to_owned(),
        after,
        limit,
    }
}

/// The ids a page carries.
fn ids(page: &RecordsPage) -> Vec<&str> {
    page.rows.iter().map(|(_, id, _)| id.as_str()).collect()
}

/// Ids compare as bytes — `B` < `Z` < `_` < `a` — whatever the database's locale says, and
/// `plugin_seq` is the plugin's last change.
#[tokio::test]
async fn records_page_is_in_byte_order_and_carries_plugin_seq() {
    let (store, client, _, _) = ready("records_page_is_in_byte_order").await;
    write_tasks(&store, &["a", "_", "Z", "B"]).await;
    let epoch = epoch_of(&client).await;

    let first = page(&store, &records(None, 100))
        .await
        .expect("read the records");
    assert_eq!(ids(&first), ["B", "Z", "_", "a"]);
    assert!(
        first
            .rows
            .iter()
            .all(|(q, _, rev)| q == "tracker.task" && *rev == 1)
    );
    assert_eq!(first.plugin_seq, Cursor { epoch, seq: 5 });
    assert_eq!(first.next, None);
}

/// A full page names its last key; the page after it continues there; a short page ends the walk,
/// and a walk that lands exactly on a page boundary ends with one empty page.
#[tokio::test]
async fn records_page_next_cursor_terminates() {
    let (store, _, _, _) = ready("records_page_next_cursor_terminates").await;
    write_tasks(&store, &["1", "2", "3", "4"]).await;

    let one = page(&store, &records(None, 3)).await.expect("page one");
    assert_eq!(ids(&one), ["1", "2", "3"]);
    let two = page(&store, &records(one.next.clone(), 3))
        .await
        .expect("page two");
    assert_eq!(ids(&two), ["4"]);
    assert_eq!(two.next, None);

    let mut after = None;
    let mut seen = Vec::new();
    for _ in 0..4 {
        let step = page(&store, &records(after, 2))
            .await
            .expect("a page of two");
        seen.extend(ids(&step).iter().map(|id| id.to_string()));
        after = step.next;
        if after.is_none() {
            break;
        }
    }
    assert_eq!(after, None, "the walk ended");
    assert_eq!(seen, ["1", "2", "3", "4"]);
}

/// A plugin with no manifest in the workspace is not found, and so is a missing workspace.
#[tokio::test]
async fn records_of_an_unknown_plugin_or_workspace_are_not_found() {
    let (store, _, _, _) = ready("records_of_an_unknown_plugin").await;
    let mut unknown = records(None, 10);
    unknown.plugin = "nope".to_owned();
    let plugin = page(&store, &unknown).await;
    assert!(
        matches!(plugin, Err(StoreError::NotFound { .. })),
        "plugin: {plugin:?}"
    );
    unknown.ws = "nope".to_owned();
    let ws = page(&store, &unknown).await;
    assert!(
        matches!(ws, Err(StoreError::NotFound { .. })),
        "workspace: {ws:?}"
    );
}

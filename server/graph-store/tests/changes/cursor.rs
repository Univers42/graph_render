//! The cursor rule: valid iff it is this epoch's and `low − 1 ≤ seq ≤ head_seq`.

use super::*;
use graph_store::changes::{CursorState, cursor_state};

/// A workspace with changes 1..=4 and changes 1 and 2 pruned, so `low` is 3.
async fn pruned(name: &str) -> (Store, u64) {
    let (store, client, _, _) = ready(name).await;
    write_tasks(&store, &["1", "2", "3"]).await;
    prune_through(&client, 2).await;
    let epoch = epoch_of(&client).await;
    (store, epoch)
}

/// A cursor below `low − 1` would skip pruned changes: gone, and the page says so.
#[tokio::test]
async fn cursor_below_low_is_gone() {
    let (store, epoch) = pruned("cursor_below_low_is_gone").await;
    let below = Cursor { epoch, seq: 1 };
    assert_eq!(
        cursor_state(&store, "ws", below).await.expect("state"),
        CursorState::Gone
    );
    let page = read(&store, &since(below)).await;
    assert!(matches!(page, Err(StoreError::Gone)), "page: {page:?}");
}

/// `low − 1` is the last pruned seq, and everything after it is still kept.
#[tokio::test]
async fn cursor_at_low_minus_one_is_valid() {
    let (store, epoch) = pruned("cursor_at_low_minus_one_is_valid").await;
    let edge = Cursor { epoch, seq: 2 };
    assert_eq!(
        cursor_state(&store, "ws", edge).await.expect("state"),
        CursorState::Valid
    );
    let page = read(&store, &since(edge))
        .await
        .expect("read from the edge");
    assert_eq!(seqs_of(&page), [3, 4]);
}

/// A cursor past the head names a change this store never wrote.
#[tokio::test]
async fn cursor_above_head_is_gone() {
    let (store, epoch) = pruned("cursor_above_head_is_gone").await;
    let ahead = Cursor { epoch, seq: 5 };
    assert_eq!(
        cursor_state(&store, "ws", ahead).await.expect("state"),
        CursorState::Gone
    );
    let head = Cursor { epoch, seq: 4 };
    assert_eq!(
        cursor_state(&store, "ws", head).await.expect("state"),
        CursorState::Valid
    );
}

/// A cursor from another epoch counts changes of a different history.
#[tokio::test]
async fn cursor_from_another_epoch_is_gone() {
    let (store, epoch) = pruned("cursor_from_another_epoch_is_gone").await;
    let other = Cursor {
        epoch: epoch + 1,
        seq: 3,
    };
    assert_eq!(
        cursor_state(&store, "ws", other).await.expect("state"),
        CursorState::Gone
    );
    let page = read(&store, &since(other)).await;
    assert!(matches!(page, Err(StoreError::Gone)), "page: {page:?}");
}

/// With nothing kept, `low` is `head_seq + 1` and only the head itself is valid.
#[tokio::test]
async fn an_emptied_log_keeps_only_the_head_valid() {
    let (store, client, _, _) = ready("an_emptied_log_keeps_only_the_head_valid").await;
    write_tasks(&store, &["1"]).await;
    prune_through(&client, 2).await;
    let epoch = epoch_of(&client).await;
    let state = |seq| cursor_state(&store, "ws", Cursor { epoch, seq });
    assert_eq!(state(2).await.expect("state"), CursorState::Valid);
    assert_eq!(state(1).await.expect("state"), CursorState::Gone);
}

/// A workspace that does not exist is not found, not gone.
#[tokio::test]
async fn a_missing_workspace_is_not_found() {
    let (store, _, _, _) = ready("changes_a_missing_workspace_is_not_found").await;
    let state = cursor_state(&store, "nope", Cursor { epoch: 1, seq: 0 }).await;
    assert!(
        matches!(state, Err(StoreError::NotFound { .. })),
        "state: {state:?}"
    );
}

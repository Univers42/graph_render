//! §4's workspace create: idempotent, drawing an epoch, and taking no seq.

use super::*;

/// A create is idempotent, draws an epoch, and a second create changes nothing at all.
///
/// WHY the three claims share one test: they are one transaction's whole observable behaviour, and
/// splitting them would mean three databases and three creates to say what one says here. The
/// epoch claim is checked on the stored row rather than on the clock: the clock advances on every
/// draw anywhere in the database, so it is the wrong instrument for "this create drew one".
#[tokio::test]
async fn workspace_create_is_idempotent_and_draws_an_epoch() {
    let (_, _, url) = support::db::fresh_pair("workspace_create_idempotent").await;
    let store = store(&url).await;
    assert!(
        store
            .create_workspace("ws", &LIMITS)
            .await
            .expect("the first create"),
        "the first create inserted, so it answers 201"
    );

    let mut client = support::db::more(&url).await;
    let epoch = epoch_of(&mut client, "ws").await;
    assert!(
        (1_700_000_000_000_000..(1u64 << 53)).contains(&epoch as u64),
        "the stored epoch {epoch} is not microseconds since 2023 below 2^53"
    );
    let seeded = doc_bytes(&mut client).await;
    assert_eq!(
        seeded,
        graph_contract::ingest::frame_bytes("ws", 0, 0) as i64,
        "an empty workspace's document is the head, the middle and the tail alone (N8)"
    );
    assert_eq!(head_of(&mut client).await, 0, "a create takes no seq");
    assert!(
        seqs(&mut client).await.is_empty(),
        "a workspace has no change log of its own"
    );

    assert!(
        !store
            .create_workspace("ws", &LIMITS)
            .await
            .expect("the second create"),
        "the second create found the row, so it answers 200"
    );
    assert_eq!(
        epoch_of(&mut client, "ws").await,
        epoch,
        "the second create stored nothing, so the epoch did not move"
    );
    assert_eq!(doc_bytes(&mut client).await, seeded, "doc_bytes moved");
}

/// The stored epoch of `ws`.
async fn epoch_of(client: &mut Client, ws: &str) -> i64 {
    client
        .query_one("SELECT epoch FROM workspaces WHERE id = $1", &[&ws])
        .await
        .expect("read the epoch")
        .get(0)
}
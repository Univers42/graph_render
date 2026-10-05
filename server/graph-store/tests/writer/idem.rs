//! §5.1 steps 2 and 7: the idempotency row, its replay, and its two refusals.
//!
//! Row `hub-idem` selects these three by name (`--test writer idem`), and `negctl-no-idem` turns
//! the lookup and the insert off so the replay case goes red.

use super::*;

/// A replay answers with the stored response, adds no seq, and does not re-apply.
#[tokio::test]
async fn idem_replay_returns_the_stored_response_and_no_seq() {
    let (store, mut client, _, _) = ready("idem_replay_returns_the_stored_response").await;
    let body = br#"{"upserts":[],"deletes":[{"collection":"task","id":"1"}]}"#;
    let first = store
        .apply_batch(&batch_write_with_key(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, r#""name":"One""#)], &[("task", "2")]),
            "key-1",
            body,
        ))
        .await
        .expect("the first request");
    assert_answer(&first, 2, 2);
    let rows = client
        .query_one("SELECT count(*) FROM idempotency WHERE ws = 'ws'", &[])
        .await
        .expect("count the idempotency rows")
        .get::<_, i64>(0);
    assert_eq!(
        rows, 1,
        "the key is stored beside the batch that produced it"
    );

    let replay = store
        .apply_batch(&batch_write_with_key(
            "ws",
            "tracker",
            batch_of(&[("task", "9", 7, r#""name":"Nine""#)], &[]),
            "key-1",
            body,
        ))
        .await
        .expect("the replay");
    assert_eq!(
        replay.response, first.response,
        "a replay answers with the stored response, whatever it was asked for"
    );
    assert_eq!(replay.seq, first.seq, "and the stored seq");
    assert_eq!(replay.applied, first.applied, "and the stored count");
    assert_eq!(head_of(&mut client).await, 2, "a replay takes no seq");
    let stored: i64 = client
        .query_one("SELECT count(*) FROM records WHERE ws = 'ws'", &[])
        .await
        .expect("count the records")
        .get(0);
    assert_eq!(
        stored, 1,
        "the replay did not apply its own body: only the first upsert is there"
    );
}

/// The same key with a different body is a 422, and nothing is stored.
#[tokio::test]
async fn idem_same_key_other_body_is_422() {
    let (store, mut client, _, _) = ready("idem_same_key_other_body_is_422").await;
    let one = br#"{"upserts":[],"deletes":[]}"#;
    let two = br#"{"upserts":[{"collection":"task","id":"1","updatedAt":1,"values":{}}]}"#;
    store
        .apply_batch(&batch_write_with_key(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
            "key-1",
            one,
        ))
        .await
        .expect("the first request");
    let before = head_of(&mut client).await;

    let error = store
        .apply_batch(&batch_write_with_key(
            "ws",
            "tracker",
            batch_of(&[("task", "2", 7, r#""name":"Two""#)], &[]),
            "key-1",
            two,
        ))
        .await
        .expect_err("the same key with another body");
    assert_eq!(
        status(&error),
        "422",
        "a reused key with another body is a 422"
    );
    assert_eq!(
        head_of(&mut client).await,
        before,
        "the refused request took no seq"
    );
    let rows: i64 = client
        .query_one("SELECT count(*) FROM idempotency WHERE ws = 'ws'", &[])
        .await
        .expect("count the idempotency rows")
        .get(0);
    assert_eq!(rows, 1, "the refused request stored no second row");
}

/// A key over 128 bytes is a 422, checked before the transaction opens.
#[tokio::test]
async fn idem_key_over_128_bytes_is_422() {
    let (store, mut client, _, _) = ready("idem_key_over_128_bytes_is_422").await;
    let key = "k".repeat(129);
    let error = store
        .apply_batch(&batch_write_with_key(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
            &key,
            b"body",
        ))
        .await
        .expect_err("a 129-byte key is past the cap");
    assert_eq!(
        status(&error),
        "422",
        "an over-long key is a 422, not a 413"
    );
    assert_eq!(
        head_of(&mut client).await,
        1,
        "the refused request stored nothing"
    );

    let at_limit = "k".repeat(128);
    store
        .apply_batch(&batch_write_with_key(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
            &at_limit,
            b"body",
        ))
        .await
        .expect("a 128-byte key is exactly at the cap");
}

/// A key is scoped per `(workspace, plugin, key)`: the same key under another plugin is another row.
#[tokio::test]
async fn idem_key_is_scoped_per_plugin() {
    let (store, client, _, _) = ready("idem_key_is_scoped_per_plugin").await;
    store
        .put_manifest(&manifest_write("ws", "other"))
        .await
        .expect("register a second plugin");
    let body = br#"{"upserts":[],"deletes":[]}"#;
    for plugin in ["tracker", "other"] {
        store
            .apply_batch(&batch_write_with_key(
                "ws",
                plugin,
                batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
                "shared",
                body,
            ))
            .await
            .unwrap_or_else(|e| panic!("{plugin}: {e}"));
    }
    let rows: i64 = client
        .query_one("SELECT count(*) FROM idempotency WHERE ws = 'ws'", &[])
        .await
        .expect("count the idempotency rows")
        .get(0);
    assert_eq!(rows, 2, "the same key under two plugins is two rows");
}

//! §5.1 steps 4 and 5: what a batch applies, what it refuses, and what it takes.

use super::*;

/// Two upserts apply and answer `{"applied":2,"seq":2}`.
///
/// WHY seq 2 and not 1: [`ready`] registers the manifest first, and a manifest that changes takes a
/// seq (§4), so the workspace's stream is already at 1 before the batch runs.
#[tokio::test]
async fn batch_applies_and_answers() {
    let (store, mut client, _, _) = ready("batch_applies_and_answers").await;
    let outcome = store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(
                &[
                    ("task", "1", 7, r#""name":"One""#),
                    ("task", "2", 7, r#""name":"Two","state":"open""#),
                ],
                &[],
            ),
        ))
        .await
        .expect("apply two upserts");
    assert_answer(&outcome, 2, 2);
    assert_eq!(
        seqs(&mut client).await,
        vec![1, 2],
        "the manifest's change and then one for the batch, whatever the batch held"
    );
    assert_eq!(
        ops(&mut client).await,
        vec![
            (2, 0, "upsert".to_owned(), "1".to_owned(), 1),
            (2, 1, "upsert".to_owned(), "2".to_owned(), 1),
        ],
        "two operations in batch order, both at rev 1"
    );
    assert_eq!(
        rev_of(&mut client, "tracker", "1").await,
        1,
        "the first write is rev 1"
    );
}

/// The same canonical text again is a no-op: no rev, no seq, no notice.
#[tokio::test]
async fn identical_upsert_takes_no_seq() {
    let (store, mut client, _, _) = ready("identical_upsert_takes_no_seq").await;
    let cells = r#""name":"Write""#;
    let first = store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, cells)], &[]),
        ))
        .await
        .expect("the first upsert");
    assert_answer(&first, 2, 1);
    let before = (
        rev_of(&mut client, "tracker", "1").await,
        head_of(&mut client).await,
    );

    let again = store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, cells)], &[]),
        ))
        .await
        .expect("the identical upsert again");
    assert_answer(&again, before.1 as u64, 0);
    assert_eq!(
        (
            rev_of(&mut client, "tracker", "1").await,
            head_of(&mut client).await
        ),
        before,
        "an identical resend moved neither the rev nor head_seq"
    );
    assert_eq!(
        seqs(&mut client).await,
        vec![1, 2],
        "an identical upsert adds no change (Review Focus 5)"
    );
    assert_eq!(
        text_of(&mut client, "tracker", "1").await,
        graph_contract::ingest::record_piece(
            &batch_of(&[("task", "1", 7, cells)], &[]).upserts[0].record("tracker")
        ),
        "the stored text is the record's canonical piece, byte for byte"
    );
}

/// `-0` against `0` and a reordered map both change the bytes and are therefore changes; the second
/// write is at rev 2 in each case.
#[tokio::test]
async fn a_changed_upsert_bumps_rev() {
    let (store, mut client, _, _) = ready("a_changed_upsert_bumps_rev").await;
    let first = r#""name":"Write","note":0"#;
    let minus_zero = r#""name":"Write","note":-0"#;
    let reordered = r#""note":0,"name":"Write""#;

    store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, first)], &[]),
        ))
        .await
        .expect("the first upsert");
    assert_eq!(rev_of(&mut client, "tracker", "1").await, 1, "rev 1");

    for (cells, what) in [(minus_zero, "-0 against 0"), (reordered, "a reordered map")] {
        let outcome = store
            .apply_batch(&batch_write(
                "ws",
                "tracker",
                batch_of(&[("task", "1", 7, cells)], &[]),
            ))
            .await
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(outcome.applied, 1, "{what} is a change");
    }
    assert_eq!(
        rev_of(&mut client, "tracker", "1").await,
        3,
        "two changes on top of rev 1"
    );
    assert_eq!(
        head_of(&mut client).await,
        4,
        "the manifest's change and three batches"
    );
    assert_eq!(
        text_of(&mut client, "tracker", "1").await,
        r#"{"collection":"tracker.task","deleted":false,"id":"1","updatedAt":7,"values":{"name":"Write"}}"#,
        "the stored text is the third write's"
    );
}

/// Deleting a record that is not there is a no-op: no rev, no seq, no operation row.
#[tokio::test]
async fn deleting_an_absent_record_is_a_noop() {
    let (store, mut client, _, _) = ready("deleting_an_absent_record_is_a_noop").await;
    let outcome = store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(&[], &[("task", "404")]),
        ))
        .await
        .expect("delete an absent record");
    assert_answer(&outcome, 1, 0);
    assert_eq!(
        (
            head_of(&mut client).await,
            seqs(&mut client).await.len() as i64
        ),
        (1, 1),
        "the manifest's seq 1 is all the stream holds, and the no-op batch did not move it"
    );
}

/// A batch whose last record names a field the manifest does not declare changes nothing.
#[tokio::test]
async fn a_batch_with_one_bad_record_changes_nothing() {
    let (store, mut client, _, _) = ready("a_batch_with_one_bad_record_changes_nothing").await;
    store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
        ))
        .await
        .expect("a good batch first");
    let before = (
        head_of(&mut client).await,
        rev_of(&mut client, "tracker", "1").await,
    );

    let error = store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(
                &[
                    ("task", "2", 7, r#""name":"Two""#),
                    ("task", "3", 7, r#""name":"Three","nope":1"#),
                ],
                &[],
            ),
        ))
        .await
        .expect_err("an undeclared field is refused");
    assert_eq!(status(&error), "422", "an undeclared field is a 422");
    assert_eq!(
        (
            head_of(&mut client).await,
            rev_of(&mut client, "tracker", "1").await
        ),
        before,
        "the refused batch stored nothing"
    );
    let stored: i64 = client
        .query_one("SELECT count(*) FROM records WHERE ws = 'ws'", &[])
        .await
        .expect("count the records")
        .get(0);
    assert_eq!(
        stored, 1,
        "the good upsert of the refused batch is not there either"
    );
}

/// An all-no-op batch still takes step 1's lock and takes no seq.
#[tokio::test]
async fn an_all_noop_batch_takes_no_seq() {
    let (store, mut client, _, _) = ready("an_all_noop_batch_takes_no_seq").await;
    let outcome = store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(&[], &[("task", "absent")]),
        ))
        .await
        .expect("an all-no-op batch");
    assert_answer(&outcome, 1, 0);
    assert_eq!(
        head_of(&mut client).await,
        1,
        "the answer quoted the manifest's seq and took no new one"
    );
    assert_eq!(
        ops(&mut client).await.len(),
        0,
        "an all-no-op batch writes no operations"
    );
    let plugin_seq: i64 = client
        .query_one("SELECT plugin_seq FROM manifests WHERE ws = 'ws'", &[])
        .await
        .expect("read plugin_seq")
        .get(0);
    assert_eq!(
        plugin_seq, 1,
        "an all-no-op batch does not move plugin_seq either"
    );
}

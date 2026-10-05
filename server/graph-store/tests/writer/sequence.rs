//! 100 concurrent batches on one workspace: `1..=100`, no duplicate, and the idempotency rows to
//! match (H6, Review Focus 5's second claim).
//!
//! This is the case `negctl-sequence-seq` turns red. The break draws the seq from a `SEQUENCE`
//! instead of the row-locked `head_seq` and drops step 1's `FOR UPDATE`, so a batch that rolls back
//! leaves a hole where the row lock would have left nothing.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::*;

/// 100 batches, one workspace, and the seqs are `1..=100` with no gap and no duplicate.
///
/// WHY 100 and not 10: a gap is a hole between two neighbours, and a hole needs enough concurrency
/// for two writers to interleave between drawing and committing. Ten writers can pass on a machine
/// that never preempts, and the control would then be green for the wrong reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn hundred_writers_have_no_gap() {
    const WRITERS: u64 = 100;
    let (store, mut client, url, _) = ready("hundred_writers_have_no_gap").await;

    // One store per writer: `Store::client` opens a connection per call, and 100 writers sharing one
    // `tokio_postgres::Client` would serialise on the driver's single connection rather than on the
    // workspace row — which is the lock this case is about.
    let barrier = Arc::new(tokio::sync::Barrier::new(WRITERS as usize));
    let mut handles = Vec::new();
    for i in 0..WRITERS {
        let store = store(&url).await;
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            let id = format!("r{i:03}");
            let body = format!(r#"{{"id":"{id}"}}"#);
            let req = batch_write_with_key(
                "ws",
                "tracker",
                batch_of(&[("task", &id, 7, &format!(r#""name":"{id}""#))], &[]),
                &format!("key-{i:03}"),
                body.as_bytes(),
            );
            barrier.wait().await;
            store.apply_batch(&req).await
        }));
    }

    let mut answers = Vec::new();
    for handle in handles {
        let outcome = handle
            .await
            .expect("a writer task did not panic")
            .unwrap_or_else(|e| panic!("a writer was refused: {e}"));
        answers.push(outcome.seq);
    }
    answers.sort_unstable();

    let stored = seqs(&mut client).await;
    assert_eq!(
        stored,
        (1..=WRITERS as i64).collect::<Vec<i64>>(),
        "the change log is exactly 1..=100: no gap, no duplicate"
    );
    let unique: BTreeSet<i64> = stored.iter().copied().collect();
    assert_eq!(
        unique.len(),
        stored.len(),
        "and no seq appears twice, which the sorted list above also shows"
    );
    assert_eq!(
        answers,
        stored,
        "every writer's answer is a seq that is in the log"
    );
    assert_eq!(head_of(&mut client).await, WRITERS as i64, "head_seq is 100");

    let rows: i64 = client
        .query_one("SELECT count(*) FROM idempotency WHERE ws = 'ws'", &[])
        .await
        .expect("count the idempotency rows")
        .get(0);
    assert_eq!(rows, 100, "every writer's key is stored beside its batch");

    let records: i64 = client
        .query_one("SELECT count(*) FROM records WHERE ws = 'ws'", &[])
        .await
        .expect("count the records")
        .get(0);
    assert_eq!(records, 100, "every writer's record is stored");
}

/// A rejected batch in the middle of the wave must not leave a hole: the rollback undoes the
/// `head_seq` bump, so the log stays contiguous.
///
/// This is the claim `sequence-seq` breaks directly, and it is here as its own case rather than
/// folded into the wave above: the wave proves 100 writers do not collide, and this proves a
/// *rollback* is what a `SEQUENCE` would turn into a hole and a row-locked counter does not.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn a_rolled_back_batch_leaves_no_gap() {
    const WRITERS: u64 = 20;
    let (store, mut client, url, _) = ready("a_rolled_back_batch_leaves_no_gap").await;

    // This one carries a key it has already used with another body, so step 2 refuses it and the
    // transaction rolls back — after step 5 would have drawn a seq under `sequence-seq`.
    let primed = store
        .apply_batch(&batch_write_with_key(
            "ws",
            "tracker",
            batch_of(&[("task", "seed", 7, r#""name":"Seed""#)], &[]),
            "used",
            b"first",
        ))
        .await
        .expect("the priming request");
    assert_answer(&primed, 2, 1);

    let barrier = Arc::new(tokio::sync::Barrier::new(WRITERS as usize));
    let mut handles = Vec::new();
    for i in 0..WRITERS {
        let good = store(&url).await;
        let bad = store(&url).await;
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            let id = format!("r{i:02}");
            barrier.wait().await;
            let ok = good
                .apply_batch(&batch_write_with_key(
                    "ws",
                    "tracker",
                    batch_of(&[("task", &id, 7, &format!(r#""name":"{id}""#))], &[]),
                    &format!("key-{i:02}"),
                    b"body",
                ))
                .await;
            let refused = bad
                .apply_batch(&batch_write_with_key(
                    "ws",
                    "tracker",
                    batch_of(&[("task", &id, 7, r#""name":"Other""#)], &[]),
                    "used",
                    b"second",
                ))
                .await;
            (ok, refused)
        }));
    }
    for handle in handles {
        let (ok, refused) = handle.await.expect("a writer task did not panic");
        ok.unwrap_or_else(|e| panic!("a good writer was refused: {e}"));
        assert_eq!(
            status(&refused.expect_err("the reused key was refused")),
            "422",
            "the reused key with another body is a 422"
        );
    }

    let stored = seqs(&mut client).await;
    assert_eq!(
        stored,
        (2..=WRITERS as i64 + 1).collect::<Vec<i64>>(),
        "the log is contiguous from the priming request's seq: {stored:?}"
    );
    assert_eq!(
        head_of(&mut client).await,
        WRITERS as i64 + 1,
        "head_seq counts the committed batches only"
    );
}
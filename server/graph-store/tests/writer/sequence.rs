//! 100 concurrent batches on one workspace: `1..=100`, no gap, no duplicate, and the idempotency
//! rows to match (H6, Review Focus 5's second claim).
//!
//! This is the case `negctl-sequence-seq` turns red. The break draws the seq from a `SEQUENCE`
//! instead of the row-locked `head_seq` and drops step 1's `FOR UPDATE`; a `SEQUENCE` is not
//! transactional, so the wave's one refused batch leaves a hole where the row-locked counter leaves
//! nothing.

use std::sync::Arc;

use super::*;

/// The `Limits` the refused writer sends, chosen so its change is over `max_change` without a large
/// body: `max_change` is `max_body + 96 × max_batch`, and a `max_batch` of 1 makes it 160 bytes.
const TIGHT: graph_contract::hub::Limits = graph_contract::hub::Limits {
    max_body: 64,
    max_batch: 1,
    max_record_bytes: 1 << 20,
};

/// 100 concurrent batches, one workspace, and the seqs are `1..=100` with no gap and no duplicate.
///
/// WHY the wave includes one batch that is refused: a `SEQUENCE` only differs from a row-locked
/// counter where a transaction rolls back, so a wave in which everything commits cannot tell the
/// two apart — and that is the whole of H6. The refused writer takes its seq and then answers 413,
/// so under the break it burns a value and leaves a hole; under the row lock it undoes the bump.
///
/// WHY 100 and not 10: a hole needs two writers to interleave between drawing and committing, and
/// ten writers can pass on a machine that never preempts — the control would then be green for the
/// wrong reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn hundred_writers_have_no_gap() {
    const WRITERS: u64 = 100;
    let (_, mut client, url, _) = ready("hundred_writers_have_no_gap").await;

    // One store per writer: `Store::client` opens a connection per call, and 100 writers sharing one
    // `tokio_postgres::Client` would serialise on the driver's single connection rather than on the
    // workspace row — which is the lock this case is about.
    let barrier = Arc::new(tokio::sync::Barrier::new(WRITERS as usize));
    let mut handles = Vec::new();
    for i in 0..WRITERS {
        let writer = store(&url).await;
        let barrier = Arc::clone(&barrier);
        handles.push(tokio::spawn(async move {
            let id = format!("r{i:03}");
            barrier.wait().await;
            if i == WRITERS / 2 {
                return refused(writer).await;
            }
            let body = format!(r#"{{"id":"{id}"}}"#);
            let req = batch_write_with_key(
                "ws",
                "tracker",
                batch_of(&[("task", &id, 7, &format!(r#""name":"{id}""#))], &[]),
                &format!("key-{i:03}"),
                body.as_bytes(),
            );
            (true, writer.apply_batch(&req).await)
        }));
    }

    let mut answers = Vec::new();
    let mut refusals = 0;
    for handle in handles {
        let (expected_ok, result) = handle.await.expect("a writer task did not panic");
        match result {
            Ok(outcome) => {
                assert!(expected_ok, "the refused writer answered {outcome:?}");
                answers.push(outcome.seq);
            }
            Err(error) => {
                assert!(!expected_ok, "a good writer was refused: {error}");
                assert_eq!(status(&error), "413", "the wave's one refusal is a 413");
                refusals += 1;
            }
        }
    }
    answers.sort_unstable();

    assert_eq!(refusals, 1, "exactly one writer in the wave is refused");
    assert_eq!(
        seqs(&mut client).await,
        (1..=WRITERS as i64).collect::<Vec<i64>>(),
        "the change log is exactly 1..=100: no gap, no duplicate"
    );
    assert_eq!(
        answers.len() as i64,
        WRITERS as i64 - 1,
        "99 writers were answered"
    );
    assert_eq!(
        answers
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        answers.len(),
        "and no two writers were given the same seq"
    );
    assert_eq!(
        head_of(&mut client).await,
        WRITERS as i64,
        "head_seq is 100"
    );

    let rows: i64 = client
        .query_one("SELECT count(*) FROM idempotency WHERE ws = 'ws'", &[])
        .await
        .expect("count the idempotency rows")
        .get(0);
    assert_eq!(
        rows, 99,
        "every answered writer's key is stored beside its batch"
    );

    let records: i64 = client
        .query_one("SELECT count(*) FROM records WHERE ws = 'ws'", &[])
        .await
        .expect("count the records")
        .get(0);
    assert_eq!(records, 99, "the refused writer's record is not among them");
}

/// A batch whose change is over `max_change`, which is refused **after** it has drawn its seq.
///
/// The refusal is what makes the control observable: `check_change` runs inside step 5, after the
/// seq, so the transaction rolls back with a value already drawn.
async fn refused(store: Store) -> (bool, Result<BatchOutcome, StoreError>) {
    let filler = "z".repeat(400);
    let mut req = batch_write(
        "ws",
        "tracker",
        batch_of(
            &[
                ("task", "too-big-1", 7, &format!(r#""name":"{filler}""#)),
                ("task", "too-big-2", 7, &format!(r#""name":"{filler}""#)),
            ],
            &[],
        ),
    );
    req.limits = TIGHT;
    (false, store.apply_batch(&req).await)
}

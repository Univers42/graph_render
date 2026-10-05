//! The forced deadlock: a 40P01 ends in a commit or a 503, never a 500 (R8, S7, §16 condition 17).
//!
//! The deadlock is built from manual SQL in the order §5.3 names: a manual `UPDATE records` locks
//! the record row and its trigger then wants the workspace row, the reverse of §5.1's step-1 order.
//! `Hooks::pause_after_lock` holds the batch between step 1 and its writes so the cycle can be
//! formed rather than raced for.

use super::*;

/// Cluster-wide settings are per CLUSTER and this crate runs its tests in parallel threads against
/// one server, so the role default this case changes must be held for its whole body.
static CLUSTER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A deadlock the batch is the victim of ends in a commit or a `retried: true` 503, and the retry
/// counter moved.
#[tokio::test]
async fn a_deadlock_ends_in_a_commit_or_a_503_never_a_500() {
    let _cluster = CLUSTER.lock().await;
    let (store, client, url, _) = ready("a_deadlock_ends_in_a_commit_or_a_503").await;

    // The victim is the *later* waiter, and which one that is depends on cost. A short
    // `deadlock_timeout` on the hub's own role is what §5.1 means by "the hub role's
    // `deadlock_timeout` makes the batch the victim": the manual session keeps PostgreSQL's 1 s
    // default by setting it back, and the batch gives up long before that and is aborted.
    client
        .batch_execute("ALTER ROLE hub SET deadlock_timeout = '200ms'")
        .await
        .expect("shorten the hub role's deadlock_timeout");
    let manual = support::db::more(&url).await;
    manual
        .batch_execute("SET deadlock_timeout = '5s'")
        .await
        .expect("the manual session keeps a long one");

    // The record the manual session will lock, written by the batch itself so no other case's setup
    // is a precondition of this one.
    store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
        ))
        .await
        .expect("write the record the manual session locks");

    let step_dir = support::step::dir();
    std::fs::create_dir_all(&step_dir).expect("create the step directory");
    let arm = format!("{step_dir}/writer-after-lock.armed");
    std::fs::write(&arm, b"").expect("arm the after-lock seam");

    let batch_store = store.clone();
    let batch = tokio::spawn(async move {
        batch_store
            .apply_batch(&batch_write(
                "ws",
                "tracker",
                batch_of(&[("task", "1", 8, r#""name":"Two""#)], &[]),
            ))
            .await
    });

    // Wait for the batch to announce it holds the workspace row.
    let ready = format!("{step_dir}/writer-after-lock.ready");
    let mut held = false;
    for _ in 0..600 {
        if std::path::Path::new(&ready).exists() {
            held = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(held, "the batch reached step 1's lock and paused there");

    // Now form the cycle: the manual session takes the record row lock and its trigger wants the
    // workspace row the batch holds. The manual statement is spawned because it blocks there.
    let cycle = support::db::more(&url).await;
    cycle
        .batch_execute("BEGIN")
        .await
        .expect("the manual session begins");
    let blocking = tokio::spawn(async move {
        let _ = cycle
            .execute(
                "UPDATE records SET updated_at = 99 WHERE ws = 'ws' AND qcoll = 'tracker.task' \
                 AND id = '1'",
                &[],
            )
            .await;
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Release the batch, which now wants the record row the manual session holds: 40P01.
    std::fs::write(format!("{step_dir}/writer-after-lock.done"), b"").expect("release the batch");
    let outcome = batch.await.expect("the batch task did not panic");
    let _ = blocking.await;

    client
        .batch_execute("ALTER ROLE hub RESET deadlock_timeout")
        .await
        .expect("restore the role default");
    let _ = std::fs::remove_file(&arm);

    match outcome {
        Ok(answer) => {
            assert!(
                answer.applied <= 1,
                "a committed answer counts one upsert: {answer:?}"
            );
            assert!(
                store.retry_count() > 0,
                "a commit after a deadlock means the one retry ran, so the counter moved"
            );
        }
        Err(error) => {
            assert!(
                matches!(error, StoreError::Serialization { retried: true }),
                "the second deadlock is `retried: true` (the hub's 503 with Retry-After: 1), \
                 not a 500: {error}"
            );
            assert!(
                store.retry_count() > 0,
                "the retry that was spent is what the counter counts"
            );
            assert_eq!(
                status(&error),
                "500-class",
                "and its code maps no other way"
            );
            assert_eq!(error.retry_after(), None, "the hub adds Retry-After itself");
        }
    }
    let _ = url;
}

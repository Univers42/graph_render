//! The forced deadlock: a 40P01 ends in a commit or a 503, never a 500 (R8, S7, §16 condition 17).
//!
//! The deadlock is built from manual SQL in the order §5.3 names: a manual `UPDATE records` locks
//! the record row and its trigger then wants the workspace row, the reverse of §5.1's step-1 order.
//! `Hooks::pause_after_lock` holds the batch between step 1 and its writes so the cycle can be
//! formed rather than raced for.

use super::*;

/// `deadlock_timeout` is a role setting here, and it is per CLUSTER, so the case holds this for its
/// whole body and every other case in the binary that changes a role default does the same.
static CLUSTER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A deadlock the batch is the victim of ends in a commit or a `retried: true` 503, and the retry
/// counter moved.
///
/// WHY the role default rather than a session `SET`: the store opens its own connection per call, so
/// there is no session to set it on. `hub-pg.sh` grants the `hub` role `SET` on `hub.writer` only,
/// so the value is installed by the superuser connection [`support::db::fresh_pair`] hands back.
#[tokio::test]
async fn a_deadlock_ends_in_a_commit_or_a_503_never_a_500() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, mut admin, url) =
        support::db::fresh_pair("a_deadlock_ends_in_a_commit_or_a_503").await;
    let store = store(&url).await;
    store
        .create_workspace("ws", &LIMITS)
        .await
        .expect("create the workspace");
    store
        .put_manifest(&manifest_write("ws", "tracker"))
        .await
        .expect("register the manifest");

    // Which side of a cycle is aborted is PostgreSQL's choice, and it picks the cheaper waiter.
    // A short `deadlock_timeout` on the hub role is how the batch becomes the one that gives up
    // (§5.1's "the hub role's `deadlock_timeout` makes the batch the victim"): the manual session
    // raises its own back to 5 s, so the batch is aborted long before the manual session would be.
    admin
        .batch_execute("ALTER ROLE hub SET deadlock_timeout = '200ms'")
        .await
        .expect("shorten the hub role's deadlock_timeout");
    // The manual session raises its own back to 5 s, and raising a GUC a role's default names needs
    // `SET` on that parameter — which `hub-pg.sh` grants for `hub.writer` only. So the case grants
    // it here and takes it back at the end, rather than the manual session silently sharing the
    // batch's 200 ms and leaving the victim to PostgreSQL's cost heuristic.
    admin
        .batch_execute("GRANT SET ON PARAMETER deadlock_timeout TO hub")
        .await
        .expect("let the hub role raise its own deadlock_timeout");

    // The record the manual session will lock, written by the batch itself so no other case's setup
    // is a precondition of this one.
    let outcome = store
        .apply_batch(&batch_write(
            "ws",
            "tracker",
            batch_of(&[("task", "1", 7, r#""name":"One""#)], &[]),
        ))
        .await;
    assert!(outcome.is_ok(), "the seeding batch applies: {outcome:?}");
    let seeded = head_of(&mut client).await;

    let step_dir = support::step::dir();
    std::fs::create_dir_all(&step_dir).expect("create the step directory");
    let arm = format!("{step_dir}/writer-after-lock.armed");
    let ready = format!("{step_dir}/writer-after-lock.ready");
    let done = format!("{step_dir}/writer-after-lock.done");
    // A previous run of this case leaves the handshake files behind, and `wait_for` only polls for
    // `.done`: a `.ready` that is already there would make the "it reached the seam" claim vacuous.
    for path in [&ready, &done] {
        let _ = std::fs::remove_file(path);
    }
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

    // Wait for the batch to announce that it holds the workspace row.
    let mut held = false;
    for _ in 0..600 {
        if std::path::Path::new(&ready).exists() {
            held = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(held, "the batch reached step 1's lock and paused there");

    // Now form the cycle: the manual session takes the record row lock and its trigger then wants
    // the workspace row the batch holds. The statement blocks there, so it runs in its own task.
    let mut cycle = support::db::more(&url).await;
    cycle
        .batch_execute("SET deadlock_timeout = '5s'")
        .await
        .expect("the manual session keeps a long one");
    cycle.batch_execute("BEGIN").await.expect("the manual session begins");
    let blocker = tokio::spawn(async move {
        cycle
            .execute(
                "UPDATE records SET updated_at = 99 WHERE ws = 'ws' AND qcoll = 'tracker.task' \
                 AND id = '1'",
                &[],
            )
            .await
    });
    // Wait for the manual statement to be *waiting on a lock*, not merely to have had time: the
    // cycle exists only once both sides are blocked, and a fixed sleep would make the case a race
    // that passes on a fast host and fails on a loaded one.
    let blocked = wait_for_lock_waiter(&mut admin).await;
    if !blocked {
        // A manual statement that *ended* rather than waiting is the case's real failure, and its
        // error says why (a refused trigger, a missing row), so it is read before anything else.
        if blocker.is_finished() {
            let ended = blocker.await.expect("the manual task did not panic");
            panic!("the manual statement ended instead of blocking: {ended:?}");
        }
        let manual_rows: i64 = client
            .query_one(
                "SELECT count(*) FROM records WHERE ws = 'ws' AND qcoll = 'tracker.task' \
                 AND id = '1'",
                &[],
            )
            .await
            .expect("count the seeded record")
            .get(0);
        let rows = admin
            .query(
                "SELECT pid, state, wait_event_type, wait_event, left(query, 60) FROM \
                 pg_stat_activity WHERE datname = current_database()",
                &[],
            )
            .await
            .expect("read the activity");
        let seen: Vec<String> = rows
            .iter()
            .map(|row| {
                format!(
                    "pid={} state={} wait={:?} query={}",
                    row.get::<_, i32>(0),
                    row.get::<_, String>(1),
                    row.get::<_, Option<String>>(2),
                    row.get::<_, String>(4)
                )
            })
            .collect();
        panic!(
            "the manual session is waiting on a lock; seeded rows: {manual_rows}; \
             activity: {seen:?}"
        );
    }

    // Release the batch, which now wants the record row the manual session holds: the cycle closes.
    std::fs::write(&done, b"").expect("release the batch");
    let outcome = batch.await.expect("the batch task did not panic");
    let manual_result = blocker.await.expect("the manual task did not panic");
    assert!(
        manual_result.is_ok(),
        "the manual statement is expected to have been the cycle's other half; it ended: \
         {manual_result:?}"
    );

    admin
        .batch_execute("ALTER ROLE hub RESET deadlock_timeout")
        .await
        .expect("restore the role default");
    admin
        .batch_execute("REVOKE SET ON PARAMETER deadlock_timeout FROM hub")
        .await
        .expect("take the parameter grant back");
    let _ = std::fs::remove_file(&arm);

    assert!(
        store.retry_count() > 0,
        "the batch was the victim of the cycle, so §5.1's one retry ran and the counter moved"
    );
    match outcome {
        Ok(answer) => {
            assert_answer(&answer, seeded as u64 + 1, 1);
            assert_eq!(
                seqs(&mut client).await,
                vec![1, seeded + 1],
                "the retry committed exactly one more change: no gap and no duplicate"
            );
        }
        Err(error) => {
            assert!(
                matches!(error, StoreError::Serialization { retried: true }),
                "a second deadlock is `retried: true` (the hub's 503 with `Retry-After: 1`), \
                 never a 500: {error}"
            );
            assert_eq!(error.retry_after(), None, "the hub adds `Retry-After` itself");
            assert_eq!(
                seqs(&mut client).await,
                vec![1, seeded],
                "the rolled-back batch left the log where it was"
            );
        }
    }
}

/// Has some backend in this database started waiting on a lock?
///
/// Caveat: it asks about the whole database rather than about the manual session's backend, so it
/// can be satisfied by another case's contention. That is why the case also holds `CLUSTER` and
/// gives every other statement in this binary its own database — and why the deadlock, not this
/// predicate, is what the assertions rest on: a false positive here makes the cycle happen a moment
/// later, and the batch is still blocked on it.
///
/// Caveat: the bound is [`HOOK_BOUND`] and not the seam's own two seconds, because a poll that
/// outlives the seam is worse than useless — the batch gives up waiting, runs its transaction
/// uncontended, commits and closes its connection, and the cycle this case is here to form never
/// exists. The seam's bound is a short one on purpose (`hooks.rs`), so the poll has to be faster.
async fn wait_for_lock_waiter(admin: &mut Client) -> bool {
    for _ in 0..HOOK_BOUND {
        let waiting: i64 = admin
            .query_one(
                "SELECT count(*) FROM pg_stat_activity WHERE datname = current_database() \
                 AND wait_event_type = 'Lock'",
                &[],
            )
            .await
            .expect("read pg_stat_activity")
            .get(0);
        if waiting > 0 {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    false
}

/// Polls of 50 ms, which is a shade under the seam's own two-second bound.
const HOOK_BOUND: u32 = 30;
//! The clock itself: microseconds, and the run-ahead being microseconds too.

use super::*;

/// An epoch is microseconds: above 1.7e15 and below 2^53, on a live database.
#[tokio::test]
async fn epoch_is_microseconds() {
    let (mut client, _, _) = support::db::fresh_pair("epoch_is_microseconds").await;
    let epoch = bump_now(&mut client).await.expect("draw an epoch");
    assert!(
        epoch > 1_700_000_000_000_000,
        "an epoch of {epoch} is not microseconds since 2023"
    );
    assert!(
        epoch < (1u64 << 53),
        "an epoch of {epoch} is not exactly representable on the wire"
    );
}

/// separates two hubs drawing in the same millisecond.
#[tokio::test]
async fn epoch_run_ahead_is_microseconds() {
    let (mut client, _, _) = support::db::fresh_pair("epoch_run_ahead_is_microseconds").await;
    client.batch_execute("BEGIN").await.expect("begin");
    let wall: i128 = client
        .query_one(
            "SELECT (extract(epoch FROM clock_timestamp()) * 1000000)::bigint",
            &[],
        )
        .await
        .expect("wall clock in microseconds")
        .get::<_, i64>(0)
        .into();
    let mut last = 0;
    for _ in 0..10 {
        last = bump_now(&mut client).await.expect("draw");
    }
    let ahead = last as i128 - wall;
    assert!(
        (1..=10_000).contains(&ahead),
        "run-ahead of {ahead} is not microseconds (10 draws, wall read before them)"
    );
    client.batch_execute("ROLLBACK").await.expect("rollback");
}

/// Deleting a workspace and recreating it draws an epoch strictly above the old one.
#[tokio::test]
async fn workspace_delete_and_recreate_draws_a_larger_epoch() {
    let (mut client, _, _) =
        support::db::fresh_pair("workspace_delete_and_recreate_draws_a_larger_epoch").await;
    make_ws(&mut client, "ws").await;
    let first = head_of(&mut client, "ws")
        .await
        .expect("head")
        .expect("a workspace")
        .0;
    client
        .execute("DELETE FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("delete");
    make_ws(&mut client, "ws").await;
    let second = head_of(&mut client, "ws")
        .await
        .expect("head")
        .expect("a workspace")
        .0;
    assert!(second > first, "recreate drew {second}, not above {first}");
}

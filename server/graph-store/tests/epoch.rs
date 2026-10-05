//! The epoch clock and the 24 triggers (spec H15, §5.3, Task 5).
#![cfg(feature = "db-tests")]

mod support;

use graph_store::epoch::{bump_now, head_of};
use tokio_postgres::Client;

/// Insert a workspace the way an operator would: outside the guard, so the trigger fires.
async fn make_ws(client: &mut Client, id: &str) {
    client
        .execute(
            "INSERT INTO workspaces (id, epoch) VALUES ($1, hub_next_epoch())",
            &[&id],
        )
        .await
        .expect("insert a workspace");
}

/// A hub write path, in §5.1 step 1's shape: an explicit transaction opening with the guard.
async fn hub_tx(client: &mut Client) {
    client.batch_execute("BEGIN").await.expect("begin");
    client
        .execute("SELECT set_config('hub.writer','1',true)", &[])
        .await
        .expect("set the writer guard");
}

/// An epoch is microseconds: above 1.7e15 and below 2^53, on a live database.
#[tokio::test]
async fn epoch_is_microseconds() {
    let (mut client, _) = support::db::fresh_pair("epoch_is_microseconds").await;
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

/// Ten draws inside one transaction leave `last` ten microseconds ahead, never ten milliseconds.
///
/// This is R6: a millisecond run-ahead would let two hubs draw the same epoch after a restore,
/// because a restore rewinds the clock's *input* by milliseconds but the run-ahead is what
/// separates two hubs drawing in the same millisecond.
#[tokio::test]
async fn epoch_run_ahead_is_microseconds() {
    let (mut client, _) = support::db::fresh_pair("epoch_run_ahead_is_microseconds").await;
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

/// One statement per event, per trigger table, each moving the epoch; `COPY` included.
#[tokio::test]
async fn manual_insert_update_delete_truncate_copy_move_the_epoch() {
    let (mut client, _) =
        support::db::fresh_pair("manual_insert_update_delete_truncate_copy_move_the_epoch").await;
    make_ws(&mut client, "ws").await;
    let mut before = head_of(&mut client, "ws").await.expect("head").expect("a workspace");

    // INSERT on manifests.
    client
        .execute(
            "INSERT INTO manifests (ws, plugin, version, text, text_bytes, decl_bytes) \
             VALUES ('ws','p',1,'{}',2,2)",
            &[],
        )
        .await
        .expect("insert a manifest");
    let after = head_of(&mut client, "ws").await.expect("head").expect("a workspace");
    assert!(after.0 > before.0, "an INSERT moved no epoch");
    before = after;

    // UPDATE on manifests.
    client
        .execute("UPDATE manifests SET version = 2", &[])
        .await
        .expect("update a manifest");
    let after = head_of(&mut client, "ws").await.expect("head").expect("a workspace");
    assert!(after.0 > before.0, "an UPDATE moved no epoch");
    before = after;

    // DELETE on manifests.
    client.execute("DELETE FROM manifests", &[]).await.expect("delete");
    let after = head_of(&mut client, "ws").await.expect("head").expect("a workspace");
    assert!(after.0 > before.0, "a DELETE moved no epoch");
    before = after;

    // COPY records FROM STDIN: one statement, so one epoch however many rows it carries.
    let mut sink = client
        .copy_in("COPY records FROM STDIN")
        .await
        .expect("open the copy sink");
    futures_util::SinkExt::send(
        &mut sink,
        "ws\tp\tp.c\tid1\t1\t7\t{}\t\\x00\t2\nws\tp\tp.c\tid2\t1\t7\t{}\t\\x00\t2\n",
    )
    .await
    .expect("copy two records");
    futures_util::SinkExt::close(&mut sink)
        .await
        .expect("close the copy sink");
    let after = head_of(&mut client, "ws").await.expect("head").expect("a workspace");
    assert!(after.0 > before.0, "a COPY moved no epoch");

    // TRUNCATE: every workspace, since a truncated table names none.
    client.execute("TRUNCATE records", &[]).await.expect("truncate");
    let after = head_of(&mut client, "ws").await.expect("head").expect("a workspace");
    assert!(after.0 > before.0, "a TRUNCATE moved no epoch");
}

/// Under `session_replication_role = replica` every event still moves the epoch.
///
/// This is why every trigger is `ENABLE ALWAYS`: a plain `ENABLE` trigger is skipped entirely
/// under the replica role, and a standby's writes would then move no epoch at all.
#[tokio::test]
async fn replica_role_write_moves_the_epoch_for_every_event() {
    let (mut client, admin) =
        support::db::fresh_pair("replica_role_write_moves_the_epoch_for_every_event").await;
    make_ws(&mut client, "ws").await;
    let baseline = head_of(&mut client, "ws").await.expect("head").expect("a workspace").0;

    // `session_replication_role` is a superuser-only GUC, so this half runs as postgres. The
    // store itself is still exercised as `hub`; only the session setting needs the privilege.
    for sql in [
        "INSERT INTO manifests (ws, plugin, version, text, text_bytes, decl_bytes) \
         VALUES ('ws','p',1,'{}',2,2)",
        "UPDATE manifests SET version = 2",
        "DELETE FROM manifests",
        "TRUNCATE manifests",
    ] {
        admin.batch_execute("SET session_replication_role = replica").await.expect("set role");
        admin.batch_execute(sql).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
        admin.batch_execute("SET session_replication_role = origin").await.expect("reset role");
    }
    let after = head_of(&mut client, "ws").await.expect("head").expect("a workspace").0;
    assert!(
        after > baseline,
        "a replica-role write moved no epoch (baseline {baseline}, after {after})"
    );
}

/// Exactly four `hub_%` triggers per trigger table, all `tgenabled = 'A'`, and none on the four
/// tables that are not workspace state.
#[tokio::test]
async fn trigger_catalog_is_exactly_four_per_table_all_always() {
    let (client, _) =
        support::db::fresh_pair("trigger_catalog_is_exactly_four_per_table_all_always").await;
    let rows = client
        .query(
            "SELECT c.relname, t.tgname, t.tgenabled FROM pg_trigger t \
             JOIN pg_class c ON c.oid = t.tgrelid \
             WHERE c.relname IN ('workspaces','manifests','records','links','change_headers',\
             'change_ops','epoch_clock','hub_meta','idempotency','hub_migrations') \
             ORDER BY c.relname, t.tgname",
            &[],
        )
        .await
        .expect("read the trigger catalog");
    let mut by_table: std::collections::BTreeMap<String, Vec<(String, String)>> =
        std::collections::BTreeMap::new();
    for row in &rows {
        by_table
            .entry(row.get::<_, String>(0))
            .or_default()
            .push((row.get(1), row.get(2)));
    }
    for table in graph_store::migrate::TRIGGER_TABLES {
        let triggers = by_table.remove(table.0).unwrap_or_default();
        assert_eq!(
            triggers.len(),
            4,
            "{} has {} triggers, not four",
            table.0,
            triggers.len()
        );
        for (name, enabled) in &triggers {
            assert!(name.starts_with("hub_"), "{} is not a hub trigger", name);
            assert_eq!(enabled, "A", "{name} on {} is not ENABLE ALWAYS", table.0);
        }
    }
    assert!(
        by_table.is_empty(),
        "triggers on tables that are not workspace state: {by_table:?}"
    );
}

/// Deleting a workspace and recreating it draws an epoch strictly above the old one.
#[tokio::test]
async fn workspace_delete_and_recreate_draws_a_larger_epoch() {
    let (mut client, _) =
        support::db::fresh_pair("workspace_delete_and_recreate_draws_a_larger_epoch").await;
    make_ws(&mut client, "ws").await;
    let first = head_of(&mut client, "ws").await.expect("head").expect("a workspace").0;
    client.execute("DELETE FROM workspaces WHERE id = 'ws'", &[]).await.expect("delete");
    make_ws(&mut client, "ws").await;
    let second = head_of(&mut client, "ws").await.expect("head").expect("a workspace").0;
    assert!(second > first, "recreate drew {second}, not above {first}");
}

/// Every hub write path moves no epoch: the guard, not the trigger's absence.
///
/// §5.1 step 1 is the shape every one of them opens with. A workspace create, a manifest PUT and
/// an all-no-op batch are three statements here rather than three Rust calls, because what is
/// under test is the guard inside the trigger — the same guard whatever wrote the statement.
#[tokio::test]
async fn hub_write_paths_move_no_epoch() {
    let (mut client, _) = support::db::fresh_pair("hub_write_paths_move_no_epoch").await;
    let mut baseline = bump_now(&mut client).await.expect("draw a baseline");
    client
        .execute(
            "INSERT INTO workspaces (id, epoch) VALUES ('ws', $1)",
            &[&(baseline as i64)],
        )
        .await
        .expect("seed a workspace");
    let seed = head_of(&mut client, "ws").await.expect("head").expect("a workspace").0;

    // A workspace create.
    hub_tx(&mut client).await;
    client
        .execute(
            "INSERT INTO workspaces (id, epoch) VALUES ('ws2', hub_next_epoch()) \
             ON CONFLICT (id) DO NOTHING",
            &[],
        )
        .await
        .expect("create a workspace");
    client.batch_execute("COMMIT").await.expect("commit");

    // A manifest PUT.
    hub_tx(&mut client).await;
    client
        .execute(
            "INSERT INTO manifests (ws, plugin, version, text, text_bytes, decl_bytes) \
             VALUES ('ws','p',1,'{}',2,2) ON CONFLICT (ws, plugin) \
             DO UPDATE SET version = 2",
            &[],
        )
        .await
        .expect("put a manifest");
    client.batch_execute("COMMIT").await.expect("commit");

    // An all-no-op batch: identical text, so the write still happens and still moves nothing.
    hub_tx(&mut client).await;
    for _ in 0..3 {
        client
            .execute(
                "INSERT INTO records (ws, plugin, qcoll, id, rev, updated_at, text, text_sha256,\
                 text_bytes) VALUES ('ws','p','p.c','r1',1,7,'{}','\\x00',2) \
                 ON CONFLICT (ws, qcoll, id) DO UPDATE SET text = records.text",
                &[],
            )
            .await
            .expect("upsert a record");
    }
    client.batch_execute("COMMIT").await.expect("commit");

    baseline = bump_now(&mut client).await.expect("draw an after");
    assert_eq!(
        baseline, seed,
        "a hub write path moved the epoch: {baseline} against the seeded {seed}"
    );
}

/// The trigger on `workspaces` does not fire on its own update.
///
/// The store's own writer bumps `head_seq` with `UPDATE workspaces SET head_seq = head_seq + 1`.
/// If that update drew an epoch, every batch would bump its own workspace's epoch and no ETag
/// would ever be stable. This runs the statement *outside* the guard, so the only thing that can
/// keep the epoch still is the depth guard on the trigger's own re-entry.
#[tokio::test]
async fn workspaces_update_does_not_bump_itself() {
    let (mut client, _) =
        support::db::fresh_pair("workspaces_update_does_not_bump_itself").await;
    make_ws(&mut client, "ws").await;
    let before = bump_now(&mut client).await.expect("draw a baseline");
    client
        .execute(
            "UPDATE workspaces SET head_seq = head_seq + 1 WHERE id = 'ws'",
            &[],
        )
        .await
        .expect("bump head_seq");
    let after = bump_now(&mut client).await.expect("draw an after");
    assert_eq!(
        before, after,
        "updating head_seq drew an epoch itself: {before} then {after}"
    );
}

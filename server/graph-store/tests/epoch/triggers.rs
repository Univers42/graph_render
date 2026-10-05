//! the 24 triggers: the catalog, and every event that must move the clock

use super::*;



/// One statement per event, per trigger table, each moving the epoch; `COPY` included.
#[tokio::test]
async fn manual_insert_update_delete_truncate_copy_move_the_epoch() {
    let (mut client, _, _) =
        support::db::fresh_pair("manual_insert_update_delete_truncate_copy_move_the_epoch").await;
    make_ws(&mut client, "ws").await;
    let mut before = clock(&mut client).await;

    // INSERT on manifests.
    client
        .execute(
            "INSERT INTO manifests (ws, plugin, version, text, text_bytes, decl_bytes) \
             VALUES ('ws','p',1,'{}',2,2)",
            &[],
        )
        .await
        .expect("insert a manifest");
    let after = clock(&mut client).await;
    assert!(after > before, "an INSERT moved no epoch");
    before = after;

    // UPDATE on manifests.
    client
        .execute("UPDATE manifests SET version = 2", &[])
        .await
        .expect("update a manifest");
    let after = clock(&mut client).await;
    assert!(after > before, "an UPDATE moved no epoch");
    before = after;

    // DELETE on manifests.
    client
        .execute("DELETE FROM manifests", &[])
        .await
        .expect("delete");
    let after = clock(&mut client).await;
    assert!(after > before, "a DELETE moved no epoch");
    before = after;

    // COPY records FROM STDIN: one statement, so one epoch however many rows it carries.
    // `&[u8]` rather than `&str`: `CopyInSink`'s item must be a `bytes::Buf`, and `bytes` is not
    // a dependency of this crate, so `&[u8]` is the one `Buf` impl reachable from here. The sink
    // is pinned because `CopyInSink` is `!Unpin`.
    let sink: tokio_postgres::CopyInSink<&[u8]> = client
        .copy_in("COPY records FROM STDIN")
        .await
        .expect("open the copy sink");
    let mut sink = Box::pin(sink);
    futures_util::SinkExt::send(
        &mut sink.as_mut(),
        &b"ws\tp\tp.c\tid1\t1\t7\t{}\t\\x616263\t2\nws\tp\tp.c\tid2\t1\t7\t{}\t\\x616263\t2\n"[..],
    )
    .await
    .expect("copy two records");
    futures_util::SinkExt::close(&mut sink.as_mut())
        .await
        .expect("close the copy sink");
    let after = clock(&mut client).await;
    assert!(after > before, "a COPY moved no epoch");

    // TRUNCATE: every workspace, since a truncated table names none.
    client
        .execute("TRUNCATE records", &[])
        .await
        .expect("truncate");
    let after = clock(&mut client).await;
    assert!(after > before, "a TRUNCATE moved no epoch");
}


/// under the replica role, and a standby's writes would then move no epoch at all.
#[tokio::test]
async fn replica_role_write_moves_the_epoch_for_every_event() {
    let (mut client, admin, _) =
        support::db::fresh_pair("replica_role_write_moves_the_epoch_for_every_event").await;
    make_ws(&mut client, "ws").await;

    // `session_replication_role` is a superuser-only GUC, so this half runs as postgres. The
    // store itself is still exercised as `hub`; only the session setting needs the privilege.
    //
    // WHY each statement is checked on its own rather than summed at the end: `one-trigger-origin`
    // puts exactly ONE trigger on a bare `ENABLE`, so a summed assertion is satisfied by the other
    // four statements moving the clock. Per-statement is what makes that control bite.
    //
    // The `records` INSERT is the statement it bites, which is why `hub_records_ins` is the one
    // that break switches.
    for sql in [
        "INSERT INTO records (ws, plugin, qcoll, id, rev, updated_at, text, text_sha256,\
         text_bytes) VALUES ('ws','p','p.c','rr',1,7,'{}','\\x616263',2)",
        "INSERT INTO manifests (ws, plugin, version, text, text_bytes, decl_bytes) \
         VALUES ('ws','p',1,'{}',2,2)",
        "UPDATE manifests SET version = 2",
        "DELETE FROM manifests",
        "TRUNCATE manifests",
    ] {
        admin
            .batch_execute("SET session_replication_role = replica")
            .await
            .expect("set role");
        let before = clock(&mut client).await;
        admin
            .batch_execute(sql)
            .await
            .unwrap_or_else(|e| panic!("{sql}: {e}"));
        admin
            .batch_execute("SET session_replication_role = origin")
            .await
            .expect("reset role");
        let after = clock(&mut client).await;
        assert!(
            after > before,
            "a replica-role write moved no epoch (baseline {before}, after {after}): {sql}"
        );
    }
}


/// tables that are not workspace state.
#[tokio::test]
async fn trigger_catalog_is_exactly_four_per_table_all_always() {
    let (client, _, _) =
        support::db::fresh_pair("trigger_catalog_is_exactly_four_per_table_all_always").await;
    let rows = client
        .query(
            "SELECT c.relname, t.tgname, t.tgenabled::text FROM pg_trigger t \
             JOIN pg_class c ON c.oid = t.tgrelid \
             WHERE c.relname IN ('workspaces','manifests','records','links','change_headers',\
             'change_ops','epoch_clock','hub_meta','idempotency','hub_migrations') \
             AND NOT t.tgisinternal ORDER BY c.relname, t.tgname",
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

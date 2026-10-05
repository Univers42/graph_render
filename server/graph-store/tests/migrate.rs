//! The migration runner and the shape of the schema it applies.
#![cfg(feature = "db-tests")]

mod support;

use graph_store::migrate;

/// Every recorded row matches the manifest, and every `applied_at` is set.
#[tokio::test]
async fn migrations_are_recorded_with_their_hashes() {
    let mut client = support::db::fresh().await;
    let rows = client
        .query("SELECT name, sha256 FROM hub_migrations ORDER BY name", &[])
        .await
        .expect("read hub_migrations");
    let recorded: Vec<(String, String)> = rows.iter().map(|r| (r.get(0), r.get(1))).collect();
    assert_eq!(recorded, migrate::manifest());
    let unset: i64 = client
        .query_one("SELECT count(*) FROM hub_migrations WHERE applied_at IS NULL", &[])
        .await
        .expect("count unset applied_at")
        .get(0);
    assert_eq!(unset, 0);
}

/// A second `apply` applies zero and changes no row.
#[tokio::test]
async fn apply_is_idempotent() {
    let mut client = support::db::fresh().await;
    assert_eq!(migrate::apply(&mut client).await.expect("re-apply"), 0);
    let before: i64 = client
        .query_one("SELECT count(*) FROM hub_migrations", &[])
        .await
        .expect("count")
        .get(0);
    assert_eq!(migrate::apply(&mut client).await.expect("re-apply"), 0);
    let after: i64 = client
        .query_one("SELECT count(*) FROM hub_migrations", &[])
        .await
        .expect("count")
        .get(0);
    assert_eq!(before, after);
}

/// A file whose bytes changed after it was applied is refused, not silently re-applied.
#[tokio::test]
async fn a_changed_migration_file_is_refused() {
    let mut client = support::db::fresh().await;
    client
        .execute(
            "UPDATE hub_migrations SET sha256 = 'deadbeef' WHERE name = '0001_schema.sql'",
            &[],
        )
        .await
        .expect("corrupt the recorded hash");
    let err = migrate::apply(&mut client).await.expect_err("must refuse");
    assert!(
        err.to_string().contains("migration hash"),
        "refused for its own reason, got: {err}"
    );
}

/// Exactly the ten tables `sql/0001_schema.sql` names, and no others.
#[tokio::test]
async fn schema_has_exactly_the_named_tables() {
    let client = support::db::fresh().await;
    let rows = client
        .query(
            "SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename",
            &[],
        )
        .await
        .expect("list tables");
    let mut got: Vec<String> = rows.iter().map(|r| r.get(0)).collect();
    got.sort();
    assert_eq!(
        got,
        [
            "change_headers",
            "change_ops",
            "epoch_clock",
            "hub_meta",
            "hub_migrations",
            "idempotency",
            "links",
            "manifests",
            "records",
            "workspaces",
        ]
    );
}

/// `records` carries exactly the nine columns §4 names, in that order.
#[tokio::test]
async fn record_columns_are_exactly() {
    let client = support::db::fresh().await;
    let rows = client
        .query(
            "SELECT attname FROM pg_attribute \
             WHERE attrelid = 'records'::regclass AND attnum > 0 AND NOT attisdropped \
             ORDER BY attnum",
            &[],
        )
        .await
        .expect("read records columns");
    let got: Vec<String> = rows.iter().map(|r| r.get(0)).collect();
    assert_eq!(
        got,
        [
            "ws",
            "plugin",
            "qcoll",
            "id",
            "rev",
            "updated_at",
            "text",
            "text_sha256",
            "text_bytes"
        ]
    );
}

/// The records primary key names `COLLATE "C"` on both scan columns, so the scan order is byte
/// order whatever the database's own collation is.
#[tokio::test]
async fn records_scan_index_is_in_byte_order() {
    let client = support::db::fresh().await;
    let def: String = client
        .query_one(
            "SELECT pg_get_indexdef(indexrelid) FROM pg_index \
             WHERE indrelid = 'records'::regclass AND indisprimary",
            &[],
        )
        .await
        .expect("read the records primary key")
        .get(0);
    assert!(def.contains("qcoll COLLATE \"C\""), "qcoll is not byte-ordered: {def}");
    assert!(def.contains("id COLLATE \"C\""), "id is not byte-ordered: {def}");
}

/// Every byte-count column is `bigint`, so a 64 MiB document and a 100 000-change log both fit.
#[tokio::test]
async fn doc_bytes_columns_are_bigint() {
    let client = support::db::fresh().await;
    let rows = client
        .query(
            "SELECT attrelid::regclass::text, attname FROM pg_attribute \
             WHERE attname IN ('doc_bytes','text_bytes','decl_bytes','plugin_bytes','bytes') \
             AND attnum > 0 AND NOT attisdropped \
             AND atttypid <> 'bigint'::regtype",
            &[],
        )
        .await
        .expect("look for a byte count that is not bigint");
    let wrong: Vec<(String, String)> = rows.iter().map(|r| (r.get(0), r.get(1))).collect();
    assert!(wrong.is_empty(), "byte counts that are not bigint: {wrong:?}");
}

/// No table stores JSON: values are the canonical text graph-contract produced.
#[tokio::test]
async fn no_table_stores_json() {
    let client = support::db::fresh().await;
    let rows = client
        .query(
            "SELECT table_name, column_name FROM information_schema.columns \
             WHERE table_schema = 'public' AND data_type = 'jsonb'",
            &[],
        )
        .await
        .expect("look for a jsonb column");
    let hits: Vec<(String, String)> = rows.iter().map(|r| (r.get(0), r.get(1))).collect();
    assert!(hits.is_empty(), "jsonb columns: {hits:?}");
}

/// `hub_migrations` carries no trigger: it is not workspace state and must never move an epoch.
#[tokio::test]
async fn hub_migrations_has_no_trigger() {
    let client = support::db::fresh().await;
    let rows = client
        .query(
            "SELECT tgname FROM pg_trigger WHERE tgrelid = 'hub_migrations'::regclass",
            &[],
        )
        .await
        .expect("read hub_migrations triggers");
    let names: Vec<String> = rows.iter().map(|r| r.get(0)).collect();
    assert!(names.is_empty(), "triggers on hub_migrations: {names:?}");
}
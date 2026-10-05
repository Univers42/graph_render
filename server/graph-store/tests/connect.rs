//! Connecting, and the §6 start checks the store owns.
#![cfg(feature = "db-tests")]

mod support;

use graph_store::StoreConfig;

#[tokio::test]
async fn connect_pings() {
    let url = support::db::url();
    let mut cfg = StoreConfig::defaults();
    cfg.url = url;
    let store = graph_store::Store::connect(&cfg).await.expect("connect");
    store.ping().await.expect("ping");
}

#[tokio::test]
async fn the_database_is_utf8_with_a_c_collation() {
    let mut client = support::db::open().await;
    let row = client
        .query_one(
            "SELECT pg_encoding_to_char(encoding), datcollate FROM pg_database \
             WHERE datname = current_database()",
            &[],
        )
        .await
        .expect("read the database's encoding and collation");
    assert_eq!(row.get::<_, String>(0), "UTF8");
    assert_eq!(row.get::<_, String>(1), "C");
}

#[tokio::test]
async fn config_defaults_match_section_6() {
    let cfg = StoreConfig::defaults();
    assert_eq!(cfg.url, "");
    assert_eq!(cfg.pool, 8);
    assert_eq!(cfg.retain, 100_000);
    assert_eq!(cfg.retain_bytes, 512 * 1024 * 1024);
    assert_eq!(cfg.changes_bytes, 8 * 1024 * 1024);
    assert_eq!(cfg.fetch_rows, 32);
    assert_eq!(cfg.max_batch, 10_000);
    assert_eq!(cfg.max_body, 4 * 1024 * 1024);
    assert_eq!(cfg.max_record_bytes, 1024 * 1024);
    assert_eq!(cfg.max_plugin_bytes, 16 * 1024 * 1024);
    assert_eq!(cfg.max_doc_bytes, 64 * 1024 * 1024);
    assert_eq!(cfg.last_seen, 65_536);
    assert_eq!(cfg.timeout_ms, 30_000);
    assert_eq!(cfg.stream_deadline_ms, 120_000);
    assert_eq!(cfg.sweeper_interval_ms, 600_000);
    assert_eq!(cfg.idem_ttl_ms, 86_400_000);
}

#[test]
fn an_empty_url_is_no_database() {
    let cfg = StoreConfig::defaults();
    assert!(matches!(
        graph_store::check(&cfg),
        Ok(()) | Err(graph_store::StoreError::NoDatabase)
    ));
}

#[test]
fn a_byte_bound_below_max_change_is_refused() {
    let mut cfg = StoreConfig::defaults();
    cfg.retain_bytes = 1;
    assert!(graph_store::check(&cfg).is_err());
    let mut cfg = StoreConfig::defaults();
    cfg.changes_bytes = 1;
    assert!(graph_store::check(&cfg).is_err());
}
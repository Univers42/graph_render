//! Reaching PostgreSQL from a test.
//!
//! Every database test reads one variable, `GM_HUB_PG_URL`, and **panics** when it is unset.
//! A skipped test is not a pass, so there is no skip path here: condition (c) is what forces the
//! `db-tests` feature instead.

/// The database URL, or a panic naming the command that sets it.
pub fn url() -> String {
    match std::env::var("GM_HUB_PG_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => panic!("set GM_HUB_PG_URL (scripts/orch/hub-pg.sh url)"),
    }
}

/// One connected client.
pub async fn open() -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(&url(), tokio_postgres::NoTls)
        .await
        .expect("connect to the hub database");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

/// A client whose schema is migrated to the current code.
pub async fn migrated() -> tokio_postgres::Client {
    let mut client = open().await;
    client
        .batch_execute("DROP SCHEMA IF EXISTS public CASCADE; CREATE SCHEMA public")
        .await
        .expect("reset the schema");
    graph_store::migrate::apply(&mut client).await.expect("migrate");
    client
}

/// Drop everything the store owns, so a case starts from nothing.
///
/// The tables are dropped in reverse dependency order, which `CASCADE` would not need but which
/// keeps the failure mode of a typo obvious.
pub async fn reset_schema(client: &mut tokio_postgres::Client) {
    client
        .batch_execute(
            "DROP TABLE IF EXISTS change_ops, change_headers, links, records, manifests, \
             idempotency, epoch_clock, hub_meta, hub_migrations, workspaces CASCADE",
        )
        .await
        .expect("drop the store's tables");
}
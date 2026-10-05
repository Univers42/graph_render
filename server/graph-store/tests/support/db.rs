//! Reaching PostgreSQL from a test.
//!
//! Every database test reads one variable, `GM_HUB_PG_URL`, and **panics** when it is unset.
//! A skipped test is not a pass, so there is no skip path here: condition (c) is what forces the
//! `db-tests` feature instead.
//!
//! Every test in this crate shares one database, so nothing here may drop the schema behind a
//! test that is still running. [`migrated`] is idempotent and never destructive; [`fresh`] takes
//! the database-wide advisory lock first, so two test binaries racing for a clean slate take
//! turns rather than corrupting each other.

/// The lock `fresh` holds. A constant, so every test binary in this crate agrees on it.
const FRESH_LOCK: i64 = 8_675_309_002;

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

/// A client whose schema is at the current code's version.
///
/// Safe to call from several tests and several test binaries at once: it applies whatever has
/// not been applied and touches nothing that has.
pub async fn migrated() -> tokio_postgres::Client {
    let mut client = open().await;
    graph_store::migrate::apply(&mut client)
        .await
        .expect("migrate to the current code");
    client
}

/// A client on an empty database: the store's tables dropped and rebuilt.
///
/// Holds the database-wide advisory lock across the drop and the migrate, so a second test
/// binary asking for the same thing waits instead of dropping the first one's tables.
///
/// Caveat: the lock is a session-level advisory lock, so it is released when this client's
/// connection closes — including when a test panics, which is the intended outcome anyway.
pub async fn fresh() -> tokio_postgres::Client {
    let mut client = open().await;
    client
        .query_one("SELECT pg_advisory_lock($1)", &[&FRESH_LOCK])
        .await
        .expect("take the fresh-schema lock");
    reset_schema(&mut client).await;
    graph_store::migrate::apply(&mut client)
        .await
        .expect("migrate to the current code");
    client
        .query_one("SELECT pg_advisory_unlock($1)", &[&FRESH_LOCK])
        .await
        .expect("release the fresh-schema lock");
    client
}

/// Drop everything the store owns, so a case starts from nothing.
///
/// The tables are dropped in reverse dependency order, which `CASCADE` covers anyway but which
/// keeps a typo's failure mode obvious.
pub async fn reset_schema(client: &mut tokio_postgres::Client) {
    client
        .batch_execute(
            "DROP TABLE IF EXISTS change_ops, change_headers, links, records, manifests, \
             idempotency, epoch_clock, hub_meta, hub_migrations, workspaces CASCADE",
        )
        .await
        .expect("drop the store's tables");
}
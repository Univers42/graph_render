//! Reaching PostgreSQL from a test.
//!
//! Every database test reads one variable, `GM_HUB_PG_URL`, and **panics** when it is unset.
//! A skipped test is not a pass, so there is no skip path here: condition (c) is what forces the
//! `db-tests` feature instead.
//!
//! # Why one database per test
//!
//! Every test in this crate shares one PostgreSQL, and `cargo test` runs a binary's tests on
//! parallel threads while several binaries can run at once. A shared schema therefore cannot be
//! reset per test: whichever test drops the tables wins the race and the other nine fail for a
//! reason that has nothing to do with what they assert. Giving each test its own database removes
//! the sharing entirely, and costs one `CREATE DATABASE` per test.

use std::sync::atomic::{AtomicU64, Ordering};

/// A per-process counter, so two tests with the same name cannot collide on a database name.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// The database URL, or a panic naming the command that sets it.
pub fn url() -> String {
    match std::env::var("GM_HUB_PG_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => panic!("set GM_HUB_PG_URL (scripts/orch/hub-pg.sh url)"),
    }
}

/// The same URL with `database` replaced, for a test's own database.
fn url_for(database: &str) -> String {
    let base = url();
    let (head, _) = base.rsplit_once('/').expect("GM_HUB_PG_URL has a database");
    format!("{head}/{database}")
}

/// One connected client on the shared database, for reads that must not disturb anything.
pub async fn open() -> tokio_postgres::Client {
    connect(&url()).await
}

/// Connect to `target`, spawning the connection task.
async fn connect(target: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(target, tokio_postgres::NoTls)
        .await
        .unwrap_or_else(|e| panic!("connect to {target}: {e}"));
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

/// The database name a test called `name` gets: letters, digits and `_` only, unique per call.
fn database_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("t_{clean}_{}_{}", std::process::id(), seq)
}

/// A client on a database that belongs to this test alone, migrated to the current code.
///
/// The database is created `C`-collated and UTF8-encoded from `template0`, so N12 holds for a
/// test's own database exactly as it does for the real one, and `locale -a` differences on the
/// host cannot change the answer.
pub async fn fresh(name: &str) -> tokio_postgres::Client {
    let database = database_name(name);
    let mut admin = open().await;
    drop_database(&mut admin, &database).await;
    admin
        .batch_execute(&format!(
            "CREATE DATABASE {database} TEMPLATE template0 ENCODING 'UTF8' \
             LC_COLLATE 'C' LC_CTYPE 'C'"
        ))
        .await
        .unwrap_or_else(|e| panic!("create {database}: {e}"));
    let mut client = connect(&url_for(&database)).await;
    graph_store::migrate::apply(&mut client)
        .await
        .unwrap_or_else(|e| panic!("migrate {database}: {e}"));
    client
}

/// Drop `database`, terminating anything still attached to it.
///
/// A leftover connection from a previous run of the same test name would otherwise make
/// `DROP DATABASE` fail, and the test would never run again.
pub async fn drop_database(client: &mut tokio_postgres::Client, database: &str) {
    let _ = client
        .execute(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
             WHERE datname = $1 AND pid <> pg_backend_pid()",
            &[&database],
        )
        .await;
    let _ = client
        .batch_execute(&format!("DROP DATABASE IF EXISTS {database}"))
        .await;
}

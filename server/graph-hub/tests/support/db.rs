//! Reaching PostgreSQL from the hub's tests.
//!
//! Every database test reads one variable, `GM_HUB_PG_URL`, and **panics** when it is unset, the
//! way `server/graph-store/tests/support/db.rs:21-26` does: a skipped test is not a pass, so there
//! is no skip path and the `db-tests` feature is what keeps `svc-test` (condition (c)) honest.
//!
//! No client crate is pinned for these tests. Every connection is reached through
//! `graph_store::Store::client`, so `server/graph-hub/Cargo.toml` names no database crate at all and
//! row `hub-breaks-off` reads a hub whose only database edge is the store. The consequence is that
//! this file never *names* the connection type: every statement runs inside an `async fn` where the
//! client is a local `let`, and a helper that would need the type in its signature is inlined
//! instead.
//!
//! `scripts/orch/hub-pg.sh url` also writes `target/hub-pg-url`, and the test process and the
//! container-level verbs share `target/`, so a test that must follow the server across a restart
//! re-reads that file. No case of Tasks 1 to 4 needs it: nothing here replaces the container.

use std::sync::atomic::{AtomicU64, Ordering};

use graph_store::{Store, StoreConfig};

/// A per-process counter, so two tests whose names sanitize to the same string cannot collide.
static SEQ: AtomicU64 = AtomicU64::new(0);

/// The database URL, or a panic naming the command that sets it.
pub fn url() -> String {
    match std::env::var("GM_HUB_PG_URL") {
        Ok(url) if !url.is_empty() => url,
        _ => panic!("set GM_HUB_PG_URL (scripts/orch/hub-pg.sh url)"),
    }
}

/// A URL nothing listens on, for the unreachable-database refusal.
///
/// Caveat: port 1 on the loopback address is refused by the kernel rather than by a listener, so the
/// failure is "cannot be reached" and never a timeout. A case that wanted a *hanging* server would
/// need a blackholed address, which this host does not offer; unreachable is what §6 checks.
pub fn dead_url() -> String {
    let base = url();
    let (head, database) = base.rsplit_once('/').expect("GM_HUB_PG_URL has a database");
    let (scheme, _) = head.split_once('@').expect("GM_HUB_PG_URL has a role");
    format!("{scheme}@127.0.0.1:1/{database}")
}

/// A store on the shared `hub` database.
pub async fn store() -> Store {
    store_on(&url()).await
}

/// A store on `url`, with every §6 default the store also holds.
pub async fn store_on(url: &str) -> Store {
    let mut config = StoreConfig::defaults();
    config.url = url.to_owned();
    Store::connect(&config)
        .await
        .expect("a store on the URL under test")
}

/// The URL of a database of this test's own, created `C`-collated and UTF8 from `template0`.
pub async fn fresh(name: &str) -> String {
    create(
        name,
        "TEMPLATE template0 ENCODING 'UTF8' LC_COLLATE 'C' LC_CTYPE 'C'",
    )
    .await
}

/// The URL of a database of this test's own that is **LATIN1** rather than UTF8, which is what the
/// encoding refusal reads.
///
/// `TEMPLATE template0` is what makes it possible at all: the `hub` database's own encoding cannot be
/// changed, and a template carrying the wrong encoding cannot hold a database with a different one.
pub async fn fresh_latin1(name: &str) -> String {
    create(
        name,
        "TEMPLATE template0 ENCODING 'LATIN1' LC_COLLATE 'C' LC_CTYPE 'C'",
    )
    .await
}

/// The URL of a database of this test's own whose collation is not `C`.
///
/// Caveat: the locale named here is the container's `C.UTF-8`, so this case depends on that locale
/// existing in `deploy/postgres.Dockerfile`'s image. Without it the create fails loudly with 22023
/// rather than quietly passing, which is the right shape for a missing fixture.
pub async fn fresh_collation(name: &str) -> String {
    create(
        name,
        "TEMPLATE template0 ENCODING 'UTF8' LC_COLLATE 'C.UTF-8' LC_CTYPE 'C.UTF-8'",
    )
    .await
}

/// Delete every change row of `ws`, which is the shape a full retention prune leaves behind.
///
/// WHY here: graph-store's retention does not exist yet (its `retain` and `retain_bytes` are read
/// by nothing), so a case that needs a cursor *below what is kept* has to make it so itself. This
/// is test SQL and never hub SQL — the hub writes none.
pub async fn drop_changes(hub: &graph_hub::app::App, ws: &str) {
    let store = hub.store().await.expect("the store under test");
    let client = store.client().await.expect("a connection");
    client
        .batch_execute(&format!(
            "DELETE FROM change_ops WHERE ws = '{ws}'; DELETE FROM change_headers WHERE ws = '{ws}'"
        ))
        .await
        .unwrap_or_else(|error| panic!("drop the change log of {ws}: {error}"));
}

/// Drop the database `url` names, so a run does not accumulate one per case.
///
/// Best effort by design: a container that is already gone has nothing left to drop, and a
/// `DROP` that fails leaves a database nobody will read.
pub async fn drop_database(url: &str) {
    let Some(database) = url.rsplit('/').next() else {
        return;
    };
    let admin = store_on(&admin_url(url)).await;
    let Ok(client) = admin.client().await else {
        return;
    };
    let _ = client
        .batch_execute(&format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
             WHERE datname = '{database}' AND pid <> pg_backend_pid()"
        ))
        .await;
    let _ = client
        .batch_execute(&format!("DROP DATABASE IF EXISTS {database}"))
        .await;
}

/// One database of this test's own, created with `options` appended to `CREATE DATABASE`.
///
/// The role is `postgres` here, not `hub`: `CREATE DATABASE` with a locale on a template is a
/// superuser's statement on some builds and a role's on others, and a test must not depend on which.
/// Every *check* still runs as `hub`, on the returned URL, exactly as the binary does.
async fn create(name: &str, options: &str) -> String {
    let database = database_name(name);
    let base = url();
    let admin = store_on(&admin_url(&base)).await;
    let client = admin.client().await.expect("an admin connection");
    let _ = client
        .batch_execute(&format!(
            "SELECT pg_terminate_backend(pid) FROM pg_stat_activity \
             WHERE datname = '{database}' AND pid <> pg_backend_pid()"
        ))
        .await;
    let _ = client
        .batch_execute(&format!("DROP DATABASE IF EXISTS {database}"))
        .await;
    client
        .batch_execute(&format!("CREATE DATABASE {database} {options}"))
        .await
        .unwrap_or_else(|error| panic!("create {database}: {error}"));
    let (head, _) = base.rsplit_once('/').expect("GM_HUB_PG_URL has a database");
    format!("{head}/{database}")
}

/// The store's migrations on the shared database, once per test process.
///
/// WHY here and not in the binary: `Store::connect` validates and builds and opens no connection,
/// so a hub that starts against an unmigrated database answers 500 on its first write. The rows run
/// `hub-pg.sh reset` before the suite, which drops the schema, so every process migrates once.
///
/// `migrate::apply` is idempotent by construction (`hub_migrations` records what ran), so the
/// `OnceCell` here is a saving and not a correctness requirement.
pub async fn migrated() {
    static ONCE: tokio::sync::OnceCell<()> = tokio::sync::OnceCell::const_new();
    ONCE.get_or_init(|| async {
        let store = store().await;
        let mut client = store
            .client()
            .await
            .expect("a connection for the migration");
        graph_store::migrate::apply(&mut client)
            .await
            .expect("the store's migrations");
    })
    .await;
}

/// Apply the store's migrations to the database `url` names, and insert one workspace row.
///
/// The hub never writes SQL, so a test that needs a workspace to *exist* writes the row itself, the
/// way `server/graph-store/tests` do. This is the subject of
/// `refusal_bytes_are_identical_with_and_without_the_workspace`: authorization must answer 403
/// without ever asking whether the workspace is there, and the only way to show that is to have it
/// there in one arm and not in the other.
pub async fn with_workspace(url: &str, ws: &str) {
    let store = store_on(url).await;
    let mut client = store
        .client()
        .await
        .expect("a connection for the migration");
    graph_store::migrate::apply(&mut client)
        .await
        .expect("the store's migrations");
    client
        .batch_execute(&format!(
            "INSERT INTO workspaces (id, epoch, head_seq) VALUES ('{ws}', 1, 0) \
             ON CONFLICT (id) DO NOTHING"
        ))
        .await
        .unwrap_or_else(|error| panic!("insert the workspace {ws}: {error}"));
}

/// The name a test called `name` gets: letters, digits and `_` only, plus a per-process counter so
/// two tests in one binary cannot collide and a reused pid cannot collide with an earlier run.
fn database_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let seq = SEQ.fetch_add(1, Ordering::Relaxed);
    format!("hub_{clean}_{}_{}", std::process::id(), seq)
}

/// The URL with its role replaced by `postgres`, for the `CREATE DATABASE` and `DROP DATABASE` of a
/// case's own database.
fn admin_url(base: &str) -> String {
    base.replacen("postgres://hub:hub@", "postgres://postgres:hub@", 1)
}

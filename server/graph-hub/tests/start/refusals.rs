//! §6's seven start refusals: five that need no connection and two that read the database.
//!
//! Every case goes through `Settings::check` and never through the binary, because a refusal only
//! `main` can produce cannot be told apart from one `main` reports for the wrong reason.

use crate::settings as read;
use crate::support::db;

/// §6's defaults over `env`, with a refusal a failure: these cases are about `Settings::check`.
fn settings(env: &[(&str, &str)]) -> graph_hub::config::Settings {
    read(env).expect("the settings under test")
}

/// §6's semaphore names at the values that make `WRITERS + READS + LAYOUTS` exactly 5, so the pool
/// case names one number per row.
fn permits_with(pool: &str) -> graph_hub::config::Settings {
    settings(&[
        ("GRAPH_HUB_WRITERS", "2"),
        ("GRAPH_HUB_READS", "2"),
        ("GRAPH_HUB_LAYOUTS", "1"),
        ("GRAPH_HUB_DB_POOL", pool),
    ])
}

/// `GRAPH_HUB_DB_POOL` at or below `WRITERS + READS + LAYOUTS` is a refusal, and one more is not.
///
/// Every permit can be held at once, so a hub whose permits cannot all be admitted starves its own
/// writes against its own reads: SSE header pages and the sweeper need what is left over.
#[tokio::test]
async fn start_check_refuses_a_pool_at_or_below_the_permits() {
    let store = db::store().await;
    for pool in ["5", "4", "1"] {
        let refusal = permits_with(pool).check(&store).await.unwrap_err();
        assert_eq!(refusal.name, "GRAPH_HUB_DB_POOL", "pool {pool}");
        assert!(
            refusal.reason.contains("GRAPH_HUB_WRITERS"),
            "the refusal names the sum it compares against: {}",
            refusal.reason
        );
    }
    assert!(
        permits_with("6").check(&store).await.is_ok(),
        "one pool connection above the permits is accepted"
    );
}

/// `GRAPH_HUB_CHANGES_BYTES` below one maximum change is a refusal, and exactly one maximum change
/// is accepted: a page that cannot hold one change is not a bound.
#[tokio::test]
async fn start_check_refuses_changes_bytes_below_max_change() {
    let store = db::store().await;
    let max = settings(&[]).store.max_change();
    let below = settings(&[("GRAPH_HUB_CHANGES_BYTES", &(max - 1).to_string())]);
    let refusal = below.check(&store).await.unwrap_err();
    assert_eq!(refusal.name, "GRAPH_HUB_CHANGES_BYTES");
    let exact = settings(&[("GRAPH_HUB_CHANGES_BYTES", &max.to_string())]);
    assert!(
        exact.check(&store).await.is_ok(),
        "a page holding exactly one maximum change is accepted"
    );
}

/// `GRAPH_HUB_RETAIN_BYTES` below one maximum change is a refusal too, for the same reason: a
/// retention bound that cannot hold one change is not a bound.
#[tokio::test]
async fn start_check_refuses_retain_bytes_below_max_change() {
    let store = db::store().await;
    let max = settings(&[]).store.max_change();
    let below = settings(&[("GRAPH_HUB_RETAIN_BYTES", &(max - 1).to_string())]);
    let refusal = below.check(&store).await.unwrap_err();
    assert_eq!(refusal.name, "GRAPH_HUB_RETAIN_BYTES");
    let exact = settings(&[("GRAPH_HUB_RETAIN_BYTES", &max.to_string())]);
    assert!(exact.check(&store).await.is_ok());
}

/// `GRAPH_HUB_MOTOR_TIMEOUT_MS` at or below 40 000 is a refusal, and 40 001 is accepted.
///
/// The floor is graph-server's `GRAPH_TIMEOUT_MS` plus its `GRAPH_BODY_TIMEOUT_MS` rounded down
/// (30 000 + 10 000): below it the hub gives up on the motor while the motor is still working, and
/// the answer would be a 502 for a request that was about to succeed.
///
/// `0` is out of `Env::millis`'s own range, so it never reaches `check`; `1` and `40 000` do, and both
/// are at or below the floor §6 names.
#[tokio::test]
async fn start_check_refuses_a_motor_timeout_at_or_below_forty_thousand() {
    let store = db::store().await;
    for ms in ["40000", "1"] {
        let below = settings(&[("GRAPH_HUB_MOTOR_TIMEOUT_MS", ms)]);
        let refusal = below.check(&store).await.unwrap_err();
        assert_eq!(refusal.name, "GRAPH_HUB_MOTOR_TIMEOUT_MS", "{ms} ms");
    }
    let above = settings(&[("GRAPH_HUB_MOTOR_TIMEOUT_MS", "40001")]);
    assert!(above.check(&store).await.is_ok());
}

/// Both credential files are required: a hub with keys and no grants answers 403 to everything and
/// reads as an authorization bug rather than a deployment mistake (Decision 7).
#[tokio::test]
async fn start_check_refuses_a_missing_keys_or_grants_file() {
    let store = db::store().await;
    for name in ["GRAPH_HUB_KEYS_FILE", "GRAPH_HUB_GRANTS_FILE"] {
        let missing = settings(&[(name, "")]);
        let refusal = missing.check(&store).await.unwrap_err();
        assert_eq!(refusal.name, name, "an empty {name} is unset");
    }
}

/// An empty `GRAPH_HUB_DB_URL` is a refusal: a hub with no database is a misconfiguration, and the
/// restore detector runs on every connection, so a silent fallback would be a hub serving nothing.
#[tokio::test]
async fn start_check_refuses_an_empty_db_url() {
    let store = db::store().await;
    let missing = settings(&[("GRAPH_HUB_DB_URL", "")]);
    let refusal = missing.check(&store).await.unwrap_err();
    assert_eq!(refusal.name, "GRAPH_HUB_DB_URL");
}

/// A database whose encoding is not UTF8 is refused, read through the store's own connection.
///
/// The check happens after `Store::connect`, which runs the restore detector (§5.3), so the refusal
/// is about the deployment and not about a query.
#[tokio::test]
async fn start_check_refuses_a_wrong_encoding() {
    let latin = db::fresh_latin1("start_encoding").await;
    let store = db::store_on(&latin).await;
    let subject = settings(&[("GRAPH_HUB_DB_URL", &latin)]);
    let refusal = subject.check(&store).await.unwrap_err();
    assert_eq!(refusal.name, "GRAPH_HUB_DB_URL");
    assert!(refusal.reason.contains("encoding"), "{}", refusal.reason);
    db::drop_database(&latin).await;
}

/// A database whose `pg_database.datcollate` is not `C` is refused, so `COLLATE "C"` order really is
/// byte order for every scan in §5.3's document order.
#[tokio::test]
async fn start_check_refuses_a_wrong_collation() {
    let other = db::fresh_collation("start_collation").await;
    let store = db::store_on(&other).await;
    let subject = settings(&[("GRAPH_HUB_DB_URL", &other)]);
    let refusal = subject.check(&store).await.unwrap_err();
    assert_eq!(refusal.name, "GRAPH_HUB_DB_URL");
    assert!(refusal.reason.contains("collation"), "{}", refusal.reason);
    db::drop_database(&other).await;
}

/// An unreachable database is a refusal naming the variable, and never the URL it could not reach.
#[tokio::test]
async fn start_check_refuses_an_unreachable_database() {
    let url = db::dead_url();
    let store = db::store_on(&url).await;
    let subject = settings(&[("GRAPH_HUB_DB_URL", &url)]);
    let refusal = subject.check(&store).await.unwrap_err();
    assert_eq!(refusal.name, "GRAPH_HUB_DB_URL");
    assert!(
        !refusal.reason.contains("postgres://"),
        "{}",
        refusal.reason
    );
    assert!(!refusal.reason.contains("127.0.0.1"), "{}", refusal.reason);
}

/// The database `scripts/orch/hub-pg.sh` starts is `C`-collated and UTF8, and every §6 default
/// passes on it. This is the case that says the refusals are refusals and not a blanket no.
#[tokio::test]
async fn start_check_passes_on_the_hub_image_database() {
    let store = db::store().await;
    settings(&[])
        .check(&store)
        .await
        .expect("the hub's own database passes every start check");
}

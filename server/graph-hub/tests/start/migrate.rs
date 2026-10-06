//! The migration a hub runs at start, after the checks and before it binds.

use graph_contract::hub::Limits;
use graph_hub::config::migrate_database;

use crate::support::db;

/// A database the hub has never seen is migrated, a second start applies nothing, and the first
/// write then succeeds: the case the SDK's live rows met as a 500 `db` on `PUT /workspaces`.
#[tokio::test]
async fn start_migrates_a_fresh_database_once() {
    let url = db::fresh_owned("start_migrate").await;
    let store = db::store_on(&url).await;
    let applied = migrate_database(&store)
        .await
        .expect("the first start migrates");
    assert_eq!(applied, graph_store::migrate::FILES.len() as u32);
    let again = migrate_database(&store)
        .await
        .expect("the second start migrates");
    assert_eq!(again, 0, "a second start re-applied a migration");
    let created = store
        .create_workspace("ws", &Limits::DEFAULT)
        .await
        .expect("a write on the migrated database");
    assert!(created, "the workspace was not created");
    db::drop_database(&url).await;
}

/// Two hubs starting on one fresh database apply each file once between them, not once each.
#[tokio::test]
async fn two_starts_at_once_apply_each_migration_once() {
    let url = db::fresh_owned("start_migrate_race").await;
    let (left, right) = (db::store_on(&url).await, db::store_on(&url).await);
    let (a, b) = tokio::join!(migrate_database(&left), migrate_database(&right));
    let total = a.expect("the left start") + b.expect("the right start");
    assert_eq!(total, graph_store::migrate::FILES.len() as u32);
    db::drop_database(&url).await;
}

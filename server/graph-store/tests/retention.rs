//! Retention and the idempotency sweeper (spec §5.1 steps 6 and the sweeper, Task 10).
//!
//! Every log is written through the store, so a case prunes what the writer stored. The direct
//! `prune` calls open the transaction themselves, as the writer does, because `prune` never opens
//! or closes one: that is the property `prune_never_runs_in_its_own_transaction` pins.
#![cfg(feature = "db-tests")]

mod support;

use graph_contract::hub::Cursor;
use graph_store::changes::ChangesReq;
use graph_store::config::StoreConfig;
use graph_store::error::StoreError;
use graph_store::{retention, sweeper};
use support::workspace::{as_writer, epoch_of, ready, store_on, write_tasks};
use tokio_postgres::Client;

/// A day, the default `GRAPH_HUB_IDEM_TTL_MS`.
const DAY_MS: u64 = 86_400_000;

/// The seqs left in the log, oldest first.
async fn seqs(client: &Client) -> Vec<u64> {
    client
        .query(
            "SELECT seq FROM change_headers WHERE ws = 'ws' ORDER BY seq",
            &[],
        )
        .await
        .expect("read the headers")
        .iter()
        .map(|row| row.get::<_, i64>(0) as u64)
        .collect()
}

/// How many rows `sql` (a `SELECT count(*) …`) counts.
async fn count(client: &Client, sql: &str) -> u64 {
    let n: i64 = client
        .query_one(sql, &[])
        .await
        .unwrap_or_else(|e| panic!("{sql}: {e}"))
        .get(0);
    n as u64
}

/// `prune` inside a writer transaction that commits, the way the batch calls it.
async fn prune_committed(client: &Client, retain: u64, retain_bytes: u64) -> u64 {
    client
        .batch_execute("BEGIN; SELECT set_config('hub.writer', '1', true)")
        .await
        .expect("open the writer transaction");
    let gone = retention::prune(client, "ws", retain, retain_bytes)
        .await
        .expect("prune");
    client
        .batch_execute("COMMIT")
        .await
        .expect("commit the prune");
    gone
}

#[tokio::test]
async fn prune_by_count_keeps_the_newest() {
    let (_, client, url, _) = ready("retention_count").await;
    let mut cfg = StoreConfig::defaults();
    cfg.retain = 3;
    let store = store_on(&url, cfg).await;
    write_tasks(&store, &["a", "b", "c", "d", "e"]).await;
    assert_eq!(
        seqs(&client).await,
        [4, 5, 6],
        "the batch's own step 6 keeps the newest three of seqs 1..=6"
    );
    let orphans = "SELECT count(*) FROM change_ops WHERE ws = 'ws' AND seq < 4";
    assert_eq!(
        count(&client, orphans).await,
        0,
        "a pruned change keeps no operation"
    );
}

#[tokio::test]
async fn prune_by_bytes_keeps_the_newest() {
    let (store, client, _, _) = ready("retention_bytes").await;
    write_tasks(&store, &["a", "b", "c", "d"]).await;
    let bytes: Vec<i64> = client
        .query(
            "SELECT bytes FROM change_headers WHERE ws = 'ws' ORDER BY seq DESC",
            &[],
        )
        .await
        .expect("read the sizes")
        .iter()
        .map(|row| row.get(0))
        .collect();
    let newest_two = (bytes[0] + bytes[1]) as u64;
    assert_eq!(prune_committed(&client, 100, newest_two).await, 3);
    assert_eq!(seqs(&client).await, [4, 5], "exactly the newest two fit");
    assert_eq!(prune_committed(&client, 100, newest_two - 1).await, 1);
    assert_eq!(
        seqs(&client).await,
        [5],
        "one byte less and only the newest fits"
    );
    assert_eq!(
        prune_committed(&client, 1, u64::MAX).await,
        0,
        "both bounds hold: nothing goes"
    );
}

#[tokio::test]
async fn prune_never_runs_in_its_own_transaction() {
    let (store, client, url, _) = ready("retention_own_tx").await;
    write_tasks(&store, &["a", "b", "c"]).await;
    let other = support::db::more(&url).await;
    client
        .batch_execute("BEGIN; SELECT set_config('hub.writer', '1', true)")
        .await
        .expect("open the writer transaction");
    let txid = "SELECT txid_current()";
    let before: i64 = client.query_one(txid, &[]).await.expect("txid").get(0);
    let gone = retention::prune(&client, "ws", 1, u64::MAX)
        .await
        .expect("prune");
    let after: i64 = client.query_one(txid, &[]).await.expect("txid").get(0);
    assert_eq!(gone, 3, "seqs 1..=3 go and seq 4 stays");
    assert_eq!(
        before, after,
        "prune returns inside the caller's transaction"
    );
    assert_eq!(
        seqs(&other).await,
        [1, 2, 3, 4],
        "a second session sees nothing pruned before the caller commits"
    );
    client.batch_execute("ROLLBACK").await.expect("roll back");
    assert_eq!(
        seqs(&other).await,
        [1, 2, 3, 4],
        "a rolled-back batch prunes nothing"
    );
}

#[tokio::test]
async fn a_pruned_cursor_is_gone() {
    let (store, client, _, _) = ready("retention_gone").await;
    write_tasks(&store, &["a", "b", "c", "d"]).await;
    prune_committed(&client, 2, u64::MAX).await;
    let epoch = epoch_of(&client).await;
    let page = |seq| ChangesReq {
        ws: "ws".to_owned(),
        since: Cursor { epoch, seq },
        limit: 100,
        max_bytes: 1 << 30,
    };
    let below = graph_store::changes::page(&store, &page(2)).await;
    assert!(
        matches!(below, Err(StoreError::Gone)),
        "seq 3 is pruned: {below:?}"
    );
    let edge = graph_store::changes::page(&store, &page(3)).await;
    assert!(edge.is_ok(), "low - 1 is still a valid cursor: {edge:?}");
}

/// `n` idempotency rows created `age` ago, keyed `<prefix><i>`.
async fn seed_keys(client: &Client, prefix: &str, n: u64, age: &str) {
    as_writer(
        client,
        &format!(
            "INSERT INTO idempotency (ws, plugin, key, body_sha256, response, seq, created_at) \
             SELECT 'ws', 'tracker', '{prefix}' || g, '\\x00'::bytea, '{{}}', 1, \
             clock_timestamp() - interval '{age}' FROM generate_series(1, {n}) g"
        ),
    )
    .await;
}

const KEYS: &str = "SELECT count(*) FROM idempotency WHERE ws = 'ws'";

#[tokio::test]
async fn the_sweeper_deletes_rows_past_24h_in_bounded_batches() {
    let (store, client, _, _) = ready("sweeper_batches").await;
    seed_keys(&client, "old", 5000, "25 hours").await;
    for call in 0..5 {
        let gone = sweeper::run(&store, DAY_MS, 1000).await.expect("sweep");
        assert_eq!(gone, 1000, "call {call} deletes exactly one batch");
        assert_eq!(count(&client, KEYS).await, 4000 - 1000 * call);
    }
    assert_eq!(sweeper::run(&store, DAY_MS, 1000).await.expect("sweep"), 0);
    assert_eq!(sweeper::interval_ms(&StoreConfig::defaults()), 600_000);
}

#[tokio::test]
async fn the_sweeper_moves_no_epoch() {
    let (store, client, _, _) = ready("sweeper_epoch").await;
    seed_keys(&client, "old", 10, "25 hours").await;
    let epoch = epoch_of(&client).await;
    assert_eq!(sweeper::run(&store, DAY_MS, 1000).await.expect("sweep"), 10);
    assert_eq!(
        epoch_of(&client).await,
        epoch,
        "the sweeper is a hub write path"
    );
}

#[tokio::test]
async fn the_sweeper_never_deletes_a_fresh_row() {
    let (store, client, _, _) = ready("sweeper_fresh").await;
    seed_keys(&client, "old", 10, "25 hours").await;
    seed_keys(&client, "new", 10, "23 hours").await;
    assert_eq!(sweeper::run(&store, DAY_MS, 1000).await.expect("sweep"), 10);
    let fresh = "SELECT count(*) FROM idempotency WHERE ws = 'ws' AND key LIKE 'new%'";
    assert_eq!(
        count(&client, fresh).await,
        10,
        "a key under a day old stays"
    );
    assert_eq!(count(&client, KEYS).await, 10, "and only the old ones went");
}

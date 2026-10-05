//! `hub-pg-durability` (spec §5.1 step 1, S4): a batch the store acknowledged survives a `kill -9`
//! of PostgreSQL, whatever the server's own `synchronous_commit` reads.
//!
//! # Shape
//!
//! Four `#[ignore]` phases, sequenced by rows `hub-pg-durability` and `negctl-sync-commit-unset`.
//! Only `scripts/orch/hub-pg.sh` can kill the container, and a `gr` test container has no `docker`:
//!
//! ```text
//! reset && start && run … durability_settings_read_on
//!   && run … acked_seqs_are_written_before_the_kill && kill && start
//!   && run … every_acked_seq_is_present_after_a_restart
//!   && sql "ALTER SYSTEM SET synchronous_commit = off" && sql "SELECT pg_reload_conf()"
//!   && run … acked_seqs_are_written_before_the_kill && kill && start
//!   && run … every_acked_seq_is_present_with_synchronous_commit_off
//! ```
//!
//! The handshake is the phase-state file: the write phase appends every seq `apply_batch` returned,
//! and the assert phases read the list back after the restart.
//!
//! WHY the second round matters more than the first: with the server at `on` every commit is
//! durable whatever the store does. Only at `off` is the store's own `set_config('synchronous_commit',
//! 'on', true)` the one thing standing between an acknowledgement and a lost batch, which is what
//! `negctl-sync-commit-unset` removes.
#![cfg(feature = "db-tests")]

mod support;

use support::fixture::{LIMITS, batch_of};
use support::workspace::{batch_write, manifest_write, store};
use support::{case, db, step};
use tokio_postgres::Client;

/// The phase-state file the four phases share.
const CASE: &str = "durability";

/// The workspace the phases write; the row's `hub` database holds nothing else.
const WS: &str = "durable";

/// Batches per write phase. Enough that the tail of a round sits in WAL buffers the WAL writer has
/// not reached yet, few enough that the default `GRAPH_HUB_RETAIN` prunes none of them.
const BATCHES: usize = 200;

/// `SHOW <name>` on a plain session, which reads the server's setting and not the writer's.
async fn show(client: &Client, name: &str) -> String {
    client
        .query_one(&format!("SHOW {name}"), &[])
        .await
        .unwrap_or_else(|e| panic!("SHOW {name}: {e}"))
        .get(0)
}

/// Every seq the write phases were acknowledged, oldest first.
fn acked() -> Vec<u64> {
    step::read(CASE)
        .iter()
        .map(|line| {
            line.parse()
                .unwrap_or_else(|_| panic!("{CASE}: not a seq: {line}"))
        })
        .collect()
}

/// Assert every acknowledged seq is in the log.
async fn assert_all_present(client: &Client) {
    let want = acked();
    assert!(
        want.len() >= BATCHES,
        "{CASE}: no write phase recorded its acks"
    );
    let rows = client
        .query("SELECT seq FROM change_headers WHERE ws = $1", &[&WS])
        .await
        .expect("read the log");
    let have: std::collections::BTreeSet<u64> =
        rows.iter().map(|row| row.get::<_, i64>(0) as u64).collect();
    let lost: Vec<u64> = want.into_iter().filter(|seq| !have.contains(seq)).collect();
    assert!(
        lost.is_empty(),
        "acknowledged and gone after the kill: {lost:?}"
    );
}

#[tokio::test]
#[ignore = "container-level: row hub-pg-durability runs this phase between hub-pg.sh verbs"]
async fn durability_settings_read_on() {
    let client = case::hub().await;
    for name in ["fsync", "full_page_writes", "synchronous_commit"] {
        assert_eq!(show(&client, name).await, "on", "{name} must read on");
    }
}

/// Create [`WS`] and register the plugin the first time; a later round writes on top. Ends with a
/// synchronous commit that flushes all of it.
///
/// WHY the fence: with the server at `off` and the store's own `on` broken, the schema and the
/// workspace would be lost with the batches, and the assert phase would fail on a missing table
/// instead of naming the acknowledged seqs that are gone. Measured 2026-10-05: without the fence
/// the negctl's assert failed on `relation "change_headers" does not exist`.
async fn ensure_workspace(client: &mut Client, store: &graph_store::Store) {
    client
        .batch_execute("SET synchronous_commit = on")
        .await
        .expect("make this session's commits durable");
    graph_store::migrate::apply(client)
        .await
        .expect("migrate the row's database");
    let known = "SELECT count(*) FROM workspaces WHERE id = $1";
    let n: i64 = client
        .query_one(known, &[&WS])
        .await
        .expect("look up")
        .get(0);
    if n == 0 {
        store.create_workspace(WS, &LIMITS).await.expect("create");
        store
            .put_manifest(&manifest_write(WS, "tracker"))
            .await
            .expect("register the manifest");
    }
    // A commit that holds an xid flushes the WAL up to its own record, the store's writes included.
    client
        .batch_execute("SELECT txid_current()")
        .await
        .expect("flush the setup");
}

#[tokio::test]
#[ignore = "container-level: row hub-pg-durability runs this phase between hub-pg.sh verbs"]
async fn acked_seqs_are_written_before_the_kill() {
    let mut client = case::hub().await;
    let store = store(&db::url_now()).await;
    ensure_workspace(&mut client, &store).await;
    let mut seqs = acked();
    let round = seqs.len();
    for i in 0..BATCHES {
        let id = format!("t{}", round + i);
        let batch = batch_of(&[("task", &id, 1, r#""name":"Task""#)], &[]);
        let outcome = store
            .apply_batch(&batch_write(WS, "tracker", batch))
            .await
            .unwrap_or_else(|e| panic!("batch {id}: {e}"));
        seqs.push(outcome.seq);
    }
    let lines: Vec<String> = seqs.iter().map(u64::to_string).collect();
    step::write(CASE, &lines);
}

#[tokio::test]
#[ignore = "container-level: row hub-pg-durability runs this phase between hub-pg.sh verbs"]
async fn every_acked_seq_is_present_after_a_restart() {
    assert_all_present(&case::hub().await).await;
}

#[tokio::test]
#[ignore = "container-level: row hub-pg-durability runs this phase between hub-pg.sh verbs"]
async fn every_acked_seq_is_present_with_synchronous_commit_off() {
    let client = case::hub().await;
    assert_eq!(
        show(&client, "synchronous_commit").await,
        "off",
        "the round this phase checks must have run with the server at off"
    );
    assert_all_present(&client).await;
}

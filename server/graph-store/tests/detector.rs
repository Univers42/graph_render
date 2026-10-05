//! The restore detector (spec §5.3, Task 6).
//!
//! # Scope of this file
//!
//! These are the cases that live entirely inside one database. The container-level ones — a
//! promotion, a point-in-time recovery, a volume snapshot, a `kill -9` — are NOT here: `scripts/orch/gr`
//! has no `docker` and no route to the Docker socket, so a test process cannot run a `hub-pg.sh`
//! verb. Those need a driver outside this container, which is a gate-row concern; see the return
//! block.
#![cfg(feature = "db-tests")]

mod support;

use std::sync::Arc;

use graph_store::pool::{Detector, DetectorOutcome, LastSeen};
use tokio_postgres::Client;

/// Cluster-wide settings (`fsync`, `full_page_writes`, a role default) are per CLUSTER, not per
/// database, and this crate runs its tests in parallel threads against one server. A case that
/// turns one off must therefore hold this for its whole body, or it silently breaks whichever
/// detector run happens to overlap it.
static CLUSTER: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A migrated database, a superuser connection to it, and its URL for extra connections.
async fn db(name: &str) -> (Client, Client, String) {
    support::db::fresh_pair(name).await
}

/// A workspace the operator created, outside the guard, so the trigger moved the clock.
async fn seed_ws(client: &mut Client, id: &str) -> u64 {
    client
        .execute(
            "INSERT INTO workspaces (id, epoch) VALUES ($1, hub_next_epoch())",
            &[&id],
        )
        .await
        .expect("insert a workspace");
    client
        .query_one("SELECT epoch FROM workspaces WHERE id = $1", &[&id])
        .await
        .expect("read the epoch")
        .get::<_, i64>(0) as u64
}

/// Run the detector on `client` with `detector`, which is what `Store::client` does per connection.
async fn run(
    detector: &Detector,
    client: &mut Client,
) -> Result<DetectorOutcome, graph_store::StoreError> {
    detector.run(client).await
}

/// A detector that has already seen this database, and the epoch to compare later runs against.
///
/// WHY the epoch is read AFTER the priming run: a fresh database has no `hub_meta` row, so the
/// first run is a BUMP by §5.3's own rule, and it moves the epoch. A baseline captured before
/// priming is therefore stale by construction, and every later assertion fails against it.
async fn primed(detector: &Detector, client: &mut Client, ws: &str) -> u64 {
    run(detector, client)
        .await
        .expect("the first run records hub_meta");
    let head: i64 = client
        .query_one("SELECT head_seq FROM workspaces WHERE id = $1", &[&ws])
        .await
        .expect("head_seq")
        .get(0);
    let epoch: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = $1", &[&ws])
        .await
        .expect("epoch")
        .get(0);
    detector.commit_watermark("0/0", ws, epoch as u64, head as u64);
    detector.set_high_water("0/0");
    epoch as u64
}

/// A detector whose high-water is unreachable, so the next run is GUARANTEED to see a mismatch.
///
/// This is how a test arranges "a bump is warranted" without having to stage a real restore.
fn stale(detector: &Detector) {
    detector.set_high_water("FFFFFFFF/FFFFFFFF");
}

/// `fsync` off: refused. `fsync` is PGC_SIGHUP, so a reload is enough.
#[tokio::test]
async fn detector_refuses_fsync_off() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, admin, _) = db("detector_refuses_fsync_off").await;
    seed_ws(&mut client, "ws").await;
    admin
        .batch_execute("ALTER SYSTEM SET fsync = off")
        .await
        .expect("set fsync off");
    admin
        .query_one("SELECT pg_reload_conf()", &[])
        .await
        .expect("reload");
    let detector = Detector::new(64);
    let mut fresh = support::db::more(&support::db::url_now()).await;
    let refused = run(&detector, &mut fresh).await;
    admin
        .batch_execute("ALTER SYSTEM RESET fsync")
        .await
        .expect("restore fsync");
    admin
        .query_one("SELECT pg_reload_conf()", &[])
        .await
        .expect("reload after reset");
    assert!(
        matches!(refused, Err(graph_store::StoreError::NoDatabase)),
        "a database with fsync off was accepted: {refused:?}"
    );
}

/// `full_page_writes` off: refused, for the same reason.
#[tokio::test]
async fn detector_refuses_full_page_writes_off() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, admin, _) = db("detector_refuses_full_page_writes_off").await;
    seed_ws(&mut client, "ws").await;
    admin
        .batch_execute("ALTER SYSTEM SET full_page_writes = off")
        .await
        .expect("set full_page_writes off");
    admin
        .query_one("SELECT pg_reload_conf()", &[])
        .await
        .expect("reload");
    let detector = Detector::new(64);
    let mut fresh = support::db::more(&support::db::url_now()).await;
    let refused = run(&detector, &mut fresh).await;
    admin
        .batch_execute("ALTER SYSTEM RESET full_page_writes")
        .await
        .expect("restore full_page_writes");
    admin
        .query_one("SELECT pg_reload_conf()", &[])
        .await
        .expect("reload after reset");
    assert!(
        matches!(refused, Err(graph_store::StoreError::NoDatabase)),
        "a database with full_page_writes off was accepted: {refused:?}"
    );
}

/// A `hub.writer` default on the hub's own role: refused.
///
/// §5.3 reads this outside a transaction, so `SET LOCAL` in the store's own writes is invisible
/// here — but a role default would suppress the epoch triggers for every write the role makes.
#[tokio::test]
async fn detector_refuses_a_hub_writer_default() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, admin, _) = db("detector_refuses_a_hub_writer_default").await;
    seed_ws(&mut client, "ws").await;
    admin
        .batch_execute("ALTER ROLE hub SET hub.writer = '1'")
        .await
        .expect("set the role default");
    let detector = Detector::new(64);
    // A NEW connection: `ALTER ROLE ... SET` is applied at login, so the session that set it
    // still has no `hub.writer` and would sail past the refusal.
    let mut fresh = support::db::more(&support::db::url_now()).await;
    let refused = run(&detector, &mut fresh).await;
    admin
        .batch_execute("ALTER ROLE hub RESET hub.writer")
        .await
        .expect("clear the role default");
    assert!(
        matches!(refused, Err(graph_store::StoreError::NoDatabase)),
        "a role with a hub.writer default was accepted: {refused:?}"
    );
}

/// No `hub_meta` row at all is a mismatch: every workspace draws a fresh epoch.
#[tokio::test]
async fn detector_bumps_on_empty_hub_meta() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, _) = db("detector_bumps_on_empty_hub_meta").await;
    let before = seed_ws(&mut client, "ws").await;
    // The seed left `hub_meta` empty, which is exactly the case: a first run has never seen this
    // database, so it cannot recognise it either.
    let detector = Detector::new(64);
    let outcome = run(&detector, &mut client).await.expect("run");
    assert!(
        matches!(outcome, DetectorOutcome::Bumped { workspaces: 1 }),
        "an empty hub_meta did not bump: {outcome:?}"
    );
    let after: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    assert!(after as u64 > before, "the bump did not move the epoch");
    // `hub_meta` is written by the same transaction, so the next run matches.
    let again = run(&detector, &mut client).await.expect("second run");
    assert_eq!(again, DetectorOutcome::Match, "the second run bumped again");
}

/// A `hub_meta` row naming a different database oid is a mismatch.
///
/// This is the `pg_restore`-into-a-new-database shape: the cluster and its timeline are the same,
/// so only the oid moves.
#[tokio::test]
async fn detector_bumps_on_a_new_database_oid() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, _) = db("detector_bumps_on_a_new_database_oid").await;
    let before = seed_ws(&mut client, "ws").await;
    let detector = Detector::new(64);
    run(&detector, &mut client).await.expect("record hub_meta");
    // Forge the row a restore into another database would carry.
    client
        .execute(
            "UPDATE hub_meta SET datoid = (datoid::bigint + 1000)::oid WHERE one",
            &[],
        )
        .await
        .expect("forge a foreign datoid");
    detector.commit_watermark("0/0", "ws", before, 0);
    let outcome = run(&detector, &mut client).await.expect("run");
    assert!(
        matches!(outcome, DetectorOutcome::Bumped { workspaces: 1 }),
        "a foreign datoid did not bump: {outcome:?}"
    );
}

/// A database that matches is left alone.
#[tokio::test]
async fn detector_match_alone_bumps_nothing() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, _) = db("detector_match_alone_bumps_nothing").await;
    seed_ws(&mut client, "ws").await;
    let detector = Detector::new(64);
    let epoch = primed(&detector, &mut client, "ws").await;
    for _ in 0..3 {
        assert_eq!(
            run(&detector, &mut client).await.expect("run"),
            DetectorOutcome::Match,
            "a matching database bumped"
        );
    }
    let after: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    assert_eq!(after as u64, epoch, "a match moved the epoch");
}

/// Four writers committing while eight connections open must not bump the epoch (Review Focus 3).
///
/// This is S2's case. The high-water is snapshotted before the LSN read and raised only under the
/// same mutex, so a writer's commit landing mid-run is not mistaken for a restore. `hw-after-lsn`
/// reverses that order and this test is what catches it.
#[tokio::test]
async fn detector_match_under_four_writers_bumps_nothing() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, url) = db("detector_match_under_four_writers_bumps_nothing").await;
    seed_ws(&mut client, "ws").await;
    let detector = Arc::new(Detector::new(64));
    let epoch = primed(&detector, &mut client, "ws").await;
    // Four writers, each raising the high-water after its own commit, exactly as §5.1 step 8 does.
    let mut writers = Vec::new();
    for id in 0..4u32 {
        let w = support::db::more(&url).await;
        let detector = Arc::clone(&detector);
        writers.push(tokio::spawn(async move {
            for round in 0..5u32 {
                w.execute(
                    "INSERT INTO links (ws, src_qcoll, src_id, field, target_qcoll, target_id) \
                     VALUES ('ws','p.c',$1,'f','p.c',$2)",
                    &[&format!("s{id}"), &format!("t{round}")],
                )
                .await
                .expect("write");
                detector.commit_watermark("0/0", "ws", epoch, 0);
            }
        }));
    }
    // Eight connections opening while they commit.
    let mut outcomes = Vec::new();
    for _ in 0..8 {
        let mut c = support::db::more(&url).await;
        outcomes.push(run(&detector, &mut c).await.expect("run"));
    }
    for w in writers {
        w.await.expect("writer");
    }
    let after: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    assert_eq!(
        after as u64, epoch,
        "the epoch moved under four writers: {outcomes:?}"
    );
}

/// Two connections opening at once bump once, not twice: the mutex is taken before the snapshot.
#[tokio::test]
async fn two_connections_opened_at_once_bump_once() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, url) = db("two_connections_opened_at_once_bump_once").await;
    seed_ws(&mut client, "ws").await;
    let detector = Arc::new(Detector::new(64));
    run(&detector, &mut client).await.expect("record hub_meta");
    // Make a bump warranted, so the two openers are really competing for one bump rather than
    // both correctly matching.
    stale(&detector);
    let mut tasks = Vec::new();
    for _ in 0..2 {
        let detector = Arc::clone(&detector);
        let url = url.clone();
        tasks.push(tokio::spawn(async move {
            let mut c = support::db::more(&url).await;
            run(&detector, &mut c).await.expect("run")
        }));
    }
    let mut bumps = 0;
    for t in tasks {
        if let DetectorOutcome::Bumped { .. } = t.await.expect("task") {
            bumps += 1;
        }
    }
    assert_eq!(bumps, 1, "two connections at once bumped {bumps} times");
}

/// The map evicts the least recently used entry once it is full.
#[tokio::test]
async fn last_seen_evicts_least_recently_used_first() {
    let mut map = LastSeen::new(2);
    map.raise("oldest", 1, 1);
    map.raise("middle", 1, 1);
    // Touch `oldest` so `middle` becomes the least recently used.
    map.raise("oldest", 2, 2);
    map.raise("newest", 1, 1);
    assert_eq!(map.len(), 2, "the map grew past its cap");
    assert!(map.get("middle").is_none(), "the middle entry survived");
    assert_eq!(
        map.get("oldest"),
        Some((2, 2)),
        "the touched entry was evicted"
    );
    assert!(map.get("newest").is_some(), "the newest entry was evicted");
}

/// Entries only rise: a lower `(epoch, seq)` is ignored, so a stale writer cannot walk back.
#[tokio::test]
async fn last_seen_entries_only_rise() {
    let mut map = LastSeen::new(8);
    map.raise("ws", 5, 9);
    map.raise("ws", 5, 3);
    assert_eq!(map.get("ws"), Some((5, 9)), "a lower seq lowered the entry");
    map.raise("ws", 6, 1);
    assert_eq!(map.get("ws"), Some((6, 1)), "a higher epoch did not rise");
}

/// A row sitting BELOW the entry this process holds for it is a mismatch on its own.
///
/// This is the case the map exists for: a volume snapshot restores every key unchanged, so neither
/// the identity nor the LSN moves, and the only signal is a workspace row that came back older
/// than the last `(epoch, head_seq)` this process committed to it. `lsn-only` skips exactly this
/// comparison, so it is the control for this leg.
#[tokio::test]
async fn a_row_below_its_map_entry_is_a_mismatch() {
    let _cluster = CLUSTER.lock().await;
    let (mut client, _, _) = db("a_row_below_its_map_entry_is_a_mismatch").await;
    seed_ws(&mut client, "ws").await;
    let detector = Detector::new(64);
    let epoch = primed(&detector, &mut client, "ws").await;
    // The map now claims a head_seq the database has never reached, and the LSN is untouched.
    detector.commit_watermark("0/0", "ws", epoch, 99);
    let outcome = run(&detector, &mut client).await.expect("run");
    assert!(
        matches!(outcome, DetectorOutcome::Bumped { workspaces: 1 }),
        "a row below its map entry did not bump: {outcome:?}"
    );
    // The bump clears the map, so the run after it matches.
    assert_eq!(
        run(&detector, &mut client).await.expect("run"),
        DetectorOutcome::Match,
        "the run after a bump bumped again"
    );
}

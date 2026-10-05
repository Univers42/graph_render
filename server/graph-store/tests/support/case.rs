//! What the four container-level detector cases share (Task 6).
//!
//! Each case is split into `#[ignore]` phase tests in its own test binary and sequenced by its
//! gate row, because only `scripts/orch/hub-pg.sh` can promote a standby, kill a container or
//! copy a volume, and a `gr` test container has no `docker`. The phases share their state through
//! one file per case ([`get`], [`put`]) and a hub that never stopped ([`hub_detector`]).
//!
//! WHY the phases use the row's own `hub` database and not a per-test one: `copy-data` snapshots
//! the data volume, so a per-test database that did not exist at snapshot time is gone after the
//! restore, and one that does must be replayable out of the WAL archive. Every container-level row
//! begins with `hub-pg.sh reset`, so the cluster is empty and the shared database is the simplest
//! thing that is also the only one that survives a volume copy.
//!
//! Caveat: `commit` writes one record and bumps `head_seq` with raw SQL, not through the writer
//! transaction, because the writer is Task 7's and this is the detector's row. It reproduces the
//! two facts the detector reads — the workspace row, and the flush LSN read after the commit — and
//! nothing else about a batch.

use graph_store::pool::{Detector, DetectorOutcome};
use tokio_postgres::Client;

use super::db;
use super::step;

/// The workspace every container-level case seeds. `^[a-z0-9][a-z0-9-]{0,62}$`, so no `_`.
pub const WS: &str = "hub-case";

/// The plugin the raw `records` rows carry; nothing reads it back.
const PLUGIN: &str = "probe";

/// Read one `key=value` field out of the case's phase state, or panic naming the case.
pub fn get(case: &str, key: &str) -> String {
    let prefix = format!("{key}=");
    step::read(case)
        .into_iter()
        .find_map(|line| line.strip_prefix(&prefix).map(str::to_string))
        .unwrap_or_else(|| panic!("{case}: the phase state has no field {key}"))
}

/// Read one field as an unsigned number.
pub fn num(case: &str, key: &str) -> u64 {
    let raw = get(case, key);
    raw.parse()
        .unwrap_or_else(|_| panic!("{case}: the field {key} is not a number: {raw}"))
}

/// Replace the case's phase state with `fields`, given as `key`, `value` pairs.
pub fn put(case: &str, fields: &[(&str, String)]) {
    let lines: Vec<String> = fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect();
    step::write(case, &lines);
}

/// A client on the row's own `hub` database, dialling the CURRENT container address.
pub async fn hub() -> Client {
    db::reopen().await
}

/// A superuser client on the same database, for the §5.3 steps that need one.
pub async fn admin() -> Client {
    db::more(&db::admin_now()).await
}

/// Create [`WS`], outside the guard, so the trigger drew its epoch. Returns that epoch.
pub async fn seed(client: &mut Client) -> u64 {
    client
        .execute(
            "INSERT INTO workspaces (id, epoch) VALUES ($1, hub_next_epoch())",
            &[&WS],
        )
        .await
        .expect("seed the workspace");
    head(client).await.0
}

/// `(epoch, head_seq)` of [`WS`], the pair the detector compares against its map.
pub async fn head(client: &mut Client) -> (u64, u64) {
    let row = client
        .query_one(
            "SELECT epoch, head_seq FROM workspaces WHERE id = $1",
            &[&WS],
        )
        .await
        .expect("read the workspace row");
    (row.get::<_, i64>(0) as u64, row.get::<_, i64>(1) as u64)
}

/// The current flush LSN as PostgreSQL prints it: `XXXX/YYYYYYYY`.
pub async fn flush(client: &mut Client) -> String {
    let row = client
        .query_one("SELECT pg_current_wal_flush_lsn()::text", &[])
        .await
        .expect("read the flush LSN");
    row.get(0)
}

/// The WAL-file timeline key §5.3 stores in `hub_meta`.
///
/// Read here WITHOUT the store's break, deliberately: the cases compare what the detector stored
/// against the spec's own expression, and a comparison that read the expression the break replaces
/// would agree with itself.
pub async fn wal_timeline(client: &mut Client) -> String {
    let row = client
        .query_one(
            "SELECT substr(pg_walfile_name(pg_current_wal_lsn()), 1, 8)",
            &[],
        )
        .await
        .expect("read the WAL-file timeline");
    row.get(0)
}

/// The other two of §5.3's three identity keys: `system_identifier` and `pg_database.oid`.
pub async fn identity(client: &mut Client) -> (u64, u32) {
    let row = client
        .query_one(
            "SELECT (pg_control_system()).system_identifier, \
             (SELECT oid FROM pg_database WHERE datname = current_database())",
            &[],
        )
        .await
        .expect("read the database identity");
    (row.get::<_, i64>(0) as u64, row.get(1))
}

/// Is the server still a standby? §5.3 refuses a database in recovery, so a case that ends its
/// row still in recovery has failed for the refusal rather than for the bump.
pub async fn in_recovery(client: &mut Client) -> bool {
    let row = client
        .query_one("SELECT pg_is_in_recovery()", &[])
        .await
        .expect("read pg_is_in_recovery");
    row.get(0)
}

/// Commit one batch and return the `(epoch, head_seq)` the workspace holds afterwards.
///
/// The transaction opens with the writer guard, so the epoch triggers stay quiet exactly as they
/// do for the real writer (§5.1 step 2), and the flush LSN is read after the `COMMIT` (§5.1
/// step 8). `id` is the record id; one record per batch.
pub async fn commit(client: &mut Client, id: &str) -> (u64, u64) {
    client.batch_execute("BEGIN").await.expect("begin");
    client
        .execute("SELECT set_config('hub.writer','1',true)", &[])
        .await
        .expect("set the writer guard");
    client
        .execute(
            "INSERT INTO records (ws, plugin, qcoll, id, rev, updated_at, text, text_sha256, \
             text_bytes) VALUES ($1, $2, 'probe.items', $3, 1, 0, '{}', \
             decode(repeat('00', 32), 'hex'), 2)",
            &[&WS, &PLUGIN, &id],
        )
        .await
        .expect("insert the batch's one record");
    client
        .execute(
            "UPDATE workspaces SET head_seq = head_seq + 1 WHERE id = $1",
            &[&WS],
        )
        .await
        .expect("draw the batch's seq");
    client.batch_execute("COMMIT").await.expect("commit");
    head(client).await
}

/// Run the detector once on a database that has never been seen, and return the epoch it left.
///
/// §5.3's first rule is a bump when `hub_meta` is empty, so the priming run is always a bump and
/// the epoch it draws is the baseline every later assertion compares against.
pub async fn prime(detector: &Detector, client: &mut Client) -> u64 {
    let outcome = detector
        .run(client)
        .await
        .expect("the priming run records hub_meta");
    assert_eq!(
        outcome,
        DetectorOutcome::Bumped { workspaces: 1 },
        "a database with no hub_meta row is a mismatch by §5.3's first rule"
    );
    head(client).await.0
}

/// The detector a hub under `case` still holds: the high-water and the map entry its last
/// committed batch left in memory, read back out of the phase state.
pub fn hub_detector(case: &str) -> Detector {
    let detector = Detector::new(64);
    detector.commit_watermark(&get(case, "hw"), WS, num(case, "epoch"), num(case, "seq"));
    detector
}

/// The record ids [`WS`] holds, in byte order — what a restore is read back against.
pub async fn record_ids(client: &mut Client) -> Vec<String> {
    let rows = client
        .query(
            "SELECT id FROM records WHERE ws = $1 ORDER BY id COLLATE \"C\"",
            &[&WS],
        )
        .await
        .expect("read the records");
    rows.iter().map(|row| row.get::<_, String>(0)).collect()
}

/// The `(system_identifier, timeline, datoid)` triple `hub_meta` holds, read back verbatim.
pub async fn stored_identity(client: &mut Client) -> (u64, String, u32) {
    let row = client
        .query_one(
            "SELECT system_identifier, timeline, datoid FROM hub_meta WHERE one",
            &[],
        )
        .await
        .expect("read hub_meta");
    (row.get::<_, i64>(0) as u64, row.get(1), row.get(2))
}

/// Checkpoint and switch the WAL, so a volume copied from this server is a clean starting point
/// whose own REDO range is already in the archive.
///
/// `copy-data` does the same through the shell; the container-level cases that kill the server
/// first need it done while the server is still up, and a test may ask for that through SQL.
/// Both statements need the `pg_checkpoint` role, which `hub` is deliberately not a member of,
/// so this opens its own superuser connection.
pub async fn quiesce() {
    let admin = admin().await;
    admin.batch_execute("CHECKPOINT").await.expect("checkpoint");
    admin
        .batch_execute("SELECT pg_switch_wal()")
        .await
        .expect("switch the WAL");
}

/// Assert the server is a primary, `why` naming what a promotion that never happened would mean.
///
/// A standby or a paused recovery is refused by §5.3 before any key is compared, so a case whose
/// row ended in recovery would prove the refusal and not the bump.
pub async fn assert_out_of_recovery(client: &mut Client, why: &str) {
    assert!(!in_recovery(client).await, "{why}");
}

/// Run the detector a hub under `case` still holds, and assert §5.3's bump.
///
/// Two things, because one is not the other: the outcome is a bump of every workspace, and the
/// bump actually moved the epoch the hub last knew.
pub async fn assert_bump(case: &str, client: &mut Client) {
    let before = num(case, "epoch");
    let outcome = hub_detector(case)
        .run(client)
        .await
        .expect("the detector runs on this server");
    assert_eq!(
        outcome,
        DetectorOutcome::Bumped { workspaces: 1 },
        "§5.3 lists five mismatch rules and this case stages one of them, so the run must bump"
    );
    assert!(
        head(client).await.0 > before,
        "the bump draws a fresh epoch, and every workspace must hold it"
    );
}

/// Assert none of §5.3's three identity keys moved since `case` recorded them, and return the
/// timeline `hub_meta` now holds.
///
/// This is what makes the high-water or the last-seen map the only signal left: if the three keys
/// are the ones the hub wrote, nothing about the database's identity changed, so §5.3's identity
/// rules cannot be what caught it.
pub async fn assert_same_identity(case: &str, client: &mut Client) -> String {
    let (sysid, timeline, datoid) = stored_identity(client).await;
    assert_eq!(
        (sysid, datoid),
        (num(case, "sysid"), num(case, "datoid") as u32),
        "the system identifier and the database oid are unchanged by every verb these four rows \
         run, so a difference here means the case staged the wrong thing"
    );
    timeline
}

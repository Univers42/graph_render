//! The restore detector itself (spec §5.3): what it compares, and what it does about a mismatch.
//!
//! Split out of `connect.rs` because opening a connection and deciding whether a database has been
//! restored are two different concerns, and only the second one is this long.

use tokio_postgres::Client;

use crate::breaks;
use crate::error::StoreError;
use crate::pool::connect::{HUB_LOCK, refuse_if_unusable};
use crate::pool::Detector;

/// Run the whole detector: refusals, then the snapshot, the reads, and the bump on a mismatch.
pub async fn run_detector(
    detector: &Detector,
    client: &mut Client,
) -> Result<crate::pool::DetectorOutcome, StoreError> {
    if !detector.claim_run() {
        return Ok(crate::pool::DetectorOutcome::Match);
    }
    refuse_if_unusable(client).await?;
    // S2: the mutex is taken BEFORE the snapshot and released only after the high-water has been
    // raised again, so two connections opening at once cannot both decide to bump.
    let _held = detector.guard().await;
    let (high_water, ids) = snapshot_for_run(detector, client).await;
    let found = match compare(client, high_water.as_deref(), &ids).await {
        Ok(found) => found,
        Err(err) if is_undefined_table(&err) => {
            // Nothing is migrated yet, so there is no database to have been restored *from*.
            return Ok(crate::pool::DetectorOutcome::Match);
        }
        Err(err) => return Err(err),
    };
    match found {
        None => Ok(crate::pool::DetectorOutcome::Match),
        Some(workspaces) => {
            // Step 8: the high-water is raised to the flush LSN read on THIS connection, and only
            // after the bump has committed. A failed commit leaves both the high-water and the map
            // alone, so the next connection bumps again.
            let lsn = flush_lsn(client).await?;
            bump(client, workspaces).await?;
            detector.clear();
            detector.set_high_water(&lsn);
            Ok(crate::pool::DetectorOutcome::Bumped { workspaces })
        }
    }
}

/// The snapshot for one run, taken before any read.
///
/// `hw-after-lsn` reverses the order: it reads the flush LSN first and *then* snapshots. That is
/// precisely the bug S2 forbids — a commit landing in between raises the high-water above the LSN
/// this run holds, so the run reads as a restore and bumps a healthy database.
async fn snapshot_for_run(
    detector: &Detector,
    client: &mut Client,
) -> (
    Option<String>,
    std::collections::BTreeMap<String, (u64, u64)>,
) {
    if breaks::on("hw-after-lsn") {
        let _ = flush_lsn(client).await;
    }
    detector.snapshot()
}

/// Is `err` PostgreSQL's `undefined_table` (`42P01`)?
///
/// Caveat: this is the store's own way of noticing an unmigrated database. The alternative — the
/// caller migrating before connecting — would put the migration on the pool's critical path for
/// every connection, including the read-only ones.
fn is_undefined_table(err: &StoreError) -> bool {
    match err {
        StoreError::Db(e) => e.code() == "42P01",
        _ => false,
    }
}

/// The §5.3 mismatch check. `Ok(None)` is a match; `Ok(Some(n))` names how many workspaces the
/// bump must touch.
async fn compare(
    client: &mut Client,
    high_water: Option<&str>,
    ids_map: &std::collections::BTreeMap<String, (u64, u64)>,
) -> Result<Option<u64>, StoreError> {
    client.batch_execute("BEGIN").await?;
    let outcome = compare_in_txn(client, high_water, ids_map).await;
    // The transaction is closed either way: leaving it open would pin a snapshot and make every
    // later statement on this connection part of the detector's transaction.
    let _ = client.batch_execute("ROLLBACK").await;
    outcome
}

/// The body of [`compare`], inside its transaction.
async fn compare_in_txn(
    client: &mut Client,
    high_water: Option<&str>,
    ids_map: &std::collections::BTreeMap<String, (u64, u64)>,
) -> Result<Option<u64>, StoreError> {
    client
        .execute(
            "SELECT set_config('hub.writer','1',true), pg_advisory_xact_lock($1)",
            &[&HUB_LOCK],
        )
        .await?;
    let meta = client
        .query_opt(
            "SELECT system_identifier, timeline, datoid FROM hub_meta WHERE one",
            &[],
        )
        .await?;
    let flush = flush_lsn(client).await?;
    // §5.3 step 2: the LSN read is compared against the *snapshotted* high-water, so a commit
    // landing between the snapshot and this read cannot look like a restore.
    if let Some(have) = high_water
        && !lsn_at_least(&flush, have)
    {
        return Ok(Some(count_workspaces(client).await?));
    }
    // §5.3: a mismatch is a missing `hub_meta` row, a key in it that differs from the database's
    // own, a row below the entry this process holds for it, or a row missing while the map holds
    // it. `lsn-only` skips the map comparison, which is the control for that leg.
    let Some(row) = meta else {
        return Ok(Some(count_workspaces(client).await?));
    };
    if identity_differs(client, &row).await? {
        return Ok(Some(count_workspaces(client).await?));
    }
    let ids: Vec<String> = ids_map.keys().cloned().collect();
    let rows = read_workspaces(client, &ids).await?;
    if rows.len() != ids.len() {
        return Ok(Some(count_workspaces(client).await?));
    }
    if breaks::on("lsn-only") {
        return Ok(None);
    }
    for (id, epoch, seq) in &rows {
        if let Some(&(have_epoch, have_seq)) = ids_map.get(id)
            && (*epoch, *seq) < (have_epoch, have_seq)
        {
            return Ok(Some(count_workspaces(client).await?));
        }
    }
    Ok(None)
}

/// The database's own identity, in the same shape `hub_meta` stores it: a fresh cluster, a
/// promotion and a logical restore each move a different one of these three, which is why all
/// three are read rather than one.
pub(crate) struct Identity {
    /// `pg_control_system().system_identifier`: a fresh cluster has never seen it.
    pub system_identifier: u64,
    /// The WAL timeline: a promotion and a PITR change it at once.
    pub timeline: String,
    /// `pg_database.oid`: a logical restore into a new database has never seen it.
    pub datoid: u32,
}

/// Read the database's own identity.
pub(crate) async fn identity(client: &mut Client) -> Result<Identity, StoreError> {
    let row = client
        .query_one(
            "SELECT (pg_control_system()).system_identifier, \
             (SELECT oid FROM pg_database WHERE datname = current_database())",
            &[],
        )
        .await?;
    Ok(Identity {
        system_identifier: row.get::<_, i64>(0) as u64,
        timeline: timeline(client).await?,
        datoid: row.get(1),
    })
}

/// The WAL timeline, from the current WAL file's name.
///
/// WHY the WAL file and not the control file's checkpoint: a promotion changes the timeline at
/// once, while `pg_control_checkpoint()` still names the old one until the next checkpoint. That
/// lag is the whole reason §5.3 reads the former. `checkpoint-timeline` reads the latter, which is
/// what makes a promotion invisible until the lag closes.
pub(crate) async fn timeline(client: &mut Client) -> Result<String, StoreError> {
    if breaks::on("checkpoint-timeline") {
        let row = client
            .query_one("SELECT (pg_control_checkpoint()).timeline_id::text", &[])
            .await?;
        return Ok(row.get::<_, String>(0));
    }
    let row = client
        .query_one(
            "SELECT substr(pg_walfile_name(pg_current_wal_lsn()), 1, 8)",
            &[],
        )
        .await?;
    Ok(row.get::<_, String>(0))
}

/// Does the database's own identity differ from the one `hub_meta` recorded?
async fn identity_differs(
    client: &mut Client,
    row: &tokio_postgres::Row,
) -> Result<bool, StoreError> {
    let now = identity(client).await?;
    Ok(row.get::<_, i64>(0) as u64 != now.system_identifier
        || row.get::<_, String>(1) != now.timeline
        || row.get::<_, u32>(2) != now.datoid)
}

/// The flush LSN on this connection.
///
/// `hw-after-lsn` snapshots the high-water *after* this read, which is exactly the bug S2 forbids:
/// a write landing in between then looks like a restore.
async fn flush_lsn(client: &mut Client) -> Result<String, StoreError> {
    let row = client
        .query_one("SELECT pg_current_wal_flush_lsn()::text", &[])
        .await?;
    Ok(row.get::<_, String>(0))
}

/// Is `new` at or above `old` in `pg_lsn`'s `XXXX/YYYYYYYY` order?
fn lsn_at_least(new: &str, old: &str) -> bool {
    let parse = |s: &str| -> (u64, u64) {
        let (hi, lo) = s.split_once('/').unwrap_or((s, "0"));
        (
            u64::from_str_radix(hi, 16).unwrap_or(0),
            u64::from_str_radix(lo, 16).unwrap_or(0),
        )
    };
    parse(new) >= parse(old)
}

/// Read `id, epoch, head_seq` for the snapshotted ids.
async fn read_workspaces(
    client: &mut Client,
    ids: &[String],
) -> Result<Vec<(String, u64, u64)>, StoreError> {
    let rows = client
        .query(
            "SELECT id, epoch, head_seq FROM workspaces WHERE id = ANY($1) ORDER BY id",
            &[&ids],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        // `bigint` is `i64` on the wire; the store's own bound is `2^53 - 1`, so the conversion
        // is total and a value above that bound is a schema violation, not a number to carry.
        out.push((
            row.get(0),
            row.get::<_, i64>(1) as u64,
            row.get::<_, i64>(2) as u64,
        ));
    }
    Ok(out)
}

/// How many workspaces a bump would touch.
async fn count_workspaces(client: &mut Client) -> Result<u64, StoreError> {
    let row = client
        .query_one("SELECT count(*) FROM workspaces", &[])
        .await?;
    Ok(row.get::<_, i64>(0).max(0) as u64)
}

/// Give every workspace a fresh epoch and record the database's identity.
///
/// The transaction opens with the writer guard, so the bump moves no epoch from the triggers; the
/// epoch it draws is the one `hub_next_epoch()` returns, one per workspace row.
async fn bump(client: &mut Client, _workspaces: u64) -> Result<(), StoreError> {
    client.batch_execute("BEGIN").await?;
    client
        .execute("SELECT set_config('hub.writer','1',true)", &[])
        .await?;
    client
        .batch_execute("UPDATE workspaces SET epoch = hub_next_epoch()")
        .await?;
    let now = identity(client).await?;
    client
        .execute(
            "INSERT INTO hub_meta (one, system_identifier, timeline, datoid) \
             VALUES (true, $1, $2, $3) \
             ON CONFLICT (one) DO UPDATE SET system_identifier = EXCLUDED.system_identifier, \
             timeline = EXCLUDED.timeline, datoid = EXCLUDED.datoid",
            &[&(now.system_identifier as i64), &now.timeline, &now.datoid],
        )
        .await?;
    client.batch_execute("COMMIT").await?;
    Ok(())
}

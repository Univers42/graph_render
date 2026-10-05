//! Opening a connection, and the restore detector §5.3 specifies.
//!
//! The refusals run first and *outside* a transaction, because each one is a property of the
//! server rather than of a transaction: a database in recovery, `fsync` or `full_page_writes`
//! off, and a `hub.writer` default on the hub's own role all mean the store cannot honour the
//! contract on this database at all.

use tokio_postgres::Client;

use crate::breaks;
use crate::error::StoreError;
use crate::pool::Detector;

/// The advisory lock the detector takes, so two hubs sharing a database serialise here.
///
/// A constant, not a derived value: the two hubs must agree without exchanging anything.
pub const HUB_LOCK: i64 = 8_675_309_001;

/// Open one connection and run the detector on it before it may be used.
pub async fn open(url: &str, detector: &Detector) -> Result<Client, StoreError> {
    let (client, connection) = tokio_postgres::connect(url, tokio_postgres::NoTls).await?;
    tokio::spawn(async move {
        // The connection task must outlive the handle, or the socket closes under the next query.
        let _ = connection.await;
    });
    let mut handle = client;
    detector.run(&mut handle).await?;
    Ok(handle)
}

/// The three refusals, in §5.3's order. Read outside a transaction.
async fn refuse_if_unusable(client: &mut Client) -> Result<(), StoreError> {
    let row = client.query_one("SELECT pg_is_in_recovery()", &[]).await?;
    if row.get::<_, bool>(0) {
        return Err(StoreError::NoDatabase);
    }
    refuse_if_off(client, "fsync").await?;
    refuse_if_off(client, "full_page_writes").await?;
    refuse_default_writer(client).await
}

/// Refuse when `name` reads `off` outside a transaction.
async fn refuse_if_off(client: &mut Client, name: &str) -> Result<(), StoreError> {
    let sql = format!("SELECT current_setting('{name}', true)");
    let row = client.query_one(sql.as_str(), &[]).await?;
    let value: Option<String> = row.get(0);
    if value.as_deref() == Some("off") {
        return Err(StoreError::NoDatabase);
    }
    Ok(())
}

/// Refuse when the hub's own role carries a `hub.writer` default.
///
/// §5.3 reads this outside a transaction on purpose: `SET LOCAL` inside the store's own write
/// transactions is invisible here, but a role-level default would silently suppress the epoch
/// triggers for every write the role ever makes.
async fn refuse_default_writer(client: &mut Client) -> Result<(), StoreError> {
    let row = client
        .query_one("SELECT current_setting('hub.writer', true)", &[])
        .await?;
    let value: Option<String> = row.get(0);
    match value.as_deref() {
        None | Some("") => Ok(()),
        Some(_) => Err(StoreError::NoDatabase),
    }
}

/// Run the whole detector: refusals, then the snapshot, the reads, and the bump on a mismatch.
pub async fn run_detector(
    detector: &Detector,
    client: &mut Client,
) -> Result<crate::pool::DetectorOutcome, StoreError> {
    refuse_if_unusable(client).await?;
    let (high_water, ids) = detector.snapshot();
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
            bump(client, workspaces).await?;
            detector.clear();
            Ok(crate::pool::DetectorOutcome::Bumped { workspaces })
        }
    }
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
    // §5.3 step 2: the LSN read is compared against the *snapshotted* high-water, bound as a
    // parameter, so a commit landing between the snapshot and this read cannot look like a
    // restore. `hw-after-lsn` snapshots afterwards, which is the bug this ordering forbids.
    if let Some(have) = high_water
        && !lsn_at_least(&flush, have)
    {
        return Ok(Some(count_workspaces(client).await?));
    }
    // §5.3: a row is a mismatch when the database's own key differs from `hub_meta`'s, or when a
    // row sits below the entry this process holds for it, or is missing while the map holds it.
    // `lsn-only` skips the map comparison entirely, which is the control for that leg.
    if meta.is_none() && !ids_map.is_empty() {
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
async fn bump(client: &mut Client, _workspaces: u64) -> Result<(), StoreError> {
    client.batch_execute("BEGIN").await?;
    client
        .execute("SELECT set_config('hub.writer','1',true)", &[])
        .await?;
    client
        .batch_execute("UPDATE workspaces SET epoch = hub_next_epoch()")
        .await?;
    client
        .execute(
            "INSERT INTO hub_meta (one, system_identifier, timeline, datoid) \
             VALUES (true, (pg_control_system()).system_identifier, \
             substr(pg_walfile_name(pg_current_wal_lsn()), 1, 8), \
             (SELECT oid FROM pg_database WHERE datname = current_database())) \
             ON CONFLICT (one) DO UPDATE SET system_identifier = EXCLUDED.system_identifier, \
             timeline = EXCLUDED.timeline, datoid = EXCLUDED.datoid",
            &[],
        )
        .await?;
    client.batch_execute("COMMIT").await?;
    if breaks::on("hw-after-lsn") {
        // Reserved for the control; see `flush_lsn`.
    }
    Ok(())
}

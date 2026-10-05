//! The four steps §5.1's writer transaction shares.
//!
//! Step 1's two settings and the row lock, step 5's seq bump, and step 8's watermark. Each is
//! here because all three entry points do them the same way; a second copy of any of them is how
//! two write paths end up with two lock orders.

use tokio_postgres::Client;
use tokio_postgres::Row;

use crate::error::StoreError;
use crate::pool::Detector;

/// A workspace row as §5.1 step 1 locked it.
pub(crate) struct Workspace {
    /// The workspace's epoch, read under the lock so every answer quotes the same one.
    pub epoch: u64,
    /// The last seq taken, which an all-no-op batch answers with.
    pub head_seq: u64,
    /// The document's byte count, which the caps are checked against under the same lock.
    pub doc_bytes: u64,
    /// How many records the workspace holds, for the frame delta in step 4.
    pub records: u64,
}

/// A seq and the instant it was taken at.
///
/// No epoch: every caller already holds the step-1 lock's [`Workspace`], and reading the epoch
/// twice would give two reads of one fact and two chances to disagree about which epoch a change
/// belongs to.
pub(crate) struct Stamp {
    /// The stream position.
    pub seq: u64,
    /// The change header's `at`, in the wire's own timestamp spelling.
    pub at: String,
}

/// §5.1 step 1: `BEGIN`, then the two settings that open every write path.
///
/// `set_config(..., true)` is `SET LOCAL`, so both settings are undone by `COMMIT` or `ROLLBACK`
/// and nothing is left on the connection for the next transaction.
///
/// Caveat: `synchronous_commit` is set **unconditionally**, not only where it reads `off` (S4). An
/// operator's `off` must not make an ack a lie, and the check would itself cost a statement on
/// the path that has to be short. `sync-commit-unset` drops the second setting, which is the only
/// way to see that it is load-bearing; the test that catches it needs a `kill -9` and is Task 10's.
pub(crate) async fn begin(client: &mut Client) -> Result<(), StoreError> {
    client.batch_execute("BEGIN").await?;
    if crate::breaks::on("sync-commit-unset") {
        client
            .execute("SELECT set_config('hub.writer','1',true)", &[])
            .await?;
        return Ok(());
    }
    client
        .execute(
            "SELECT set_config('hub.writer','1',true), set_config('synchronous_commit','on',true)",
            &[],
        )
        .await?;
    Ok(())
}

/// §5.1 step 1's lock: the workspace row `FOR UPDATE`, or [`StoreError::NotFound`].
///
/// This is the lock every writer to one workspace serializes on (H6), and the first lock the
/// manifest PUT takes as well (N9). The materializer takes it second and the changes reader takes
/// it on its snapshot's own terms, so no pair of them can cycle (§5.3).
///
/// Caveat: `sequence-seq` drops the `FOR UPDATE` and draws the seq from a `SEQUENCE` instead,
/// which is what §5.1 forbids. The dropped lock is half the control: without it the batch's own
/// `UPDATE workspaces` and the change rows interleave freely.
pub(crate) async fn lock_workspace(client: &mut Client, ws: &str) -> Result<Workspace, StoreError> {
    read_workspace(client, ws, !crate::breaks::on("sequence-seq")).await
}

/// The row read, with or without the lock.
async fn read_workspace(
    client: &mut Client,
    ws: &str,
    lock: bool,
) -> Result<Workspace, StoreError> {
    // The record count rides along on the same row lock. It is a second read because `doc_bytes`'s
    // frame depends on it and `doc_bytes` is the column the caps check; reading it under the same
    // lock is what keeps a concurrent batch from moving one between the two.
    let sql = if lock {
        "SELECT epoch, head_seq, doc_bytes, (SELECT count(*) FROM records WHERE ws = $1) \
         FROM workspaces WHERE id = $1 FOR UPDATE"
    } else {
        "SELECT epoch, head_seq, doc_bytes, (SELECT count(*) FROM records WHERE ws = $1) \
         FROM workspaces WHERE id = $1"
    };
    let row = client.query_opt(sql, &[&ws]).await?;
    row.as_ref().map(workspace_of).ok_or_else(|| StoreError::NotFound {
        what: format!("workspace `{ws}`"),
    })
}

/// Four columns to one struct, with the driver's `i64` narrowed to the store's `u64`.
///
/// The narrowing is `as u64`, not a checked conversion, because every column is `bigint NOT NULL`
/// with a `CHECK` at zero or above, and a value below zero would have been refused by the row that
/// holds it. `count(*)` is the exception the schema does not cover, and it is never negative.
fn workspace_of(row: &Row) -> Workspace {
    Workspace {
        epoch: row.get::<_, i64>(0) as u64,
        head_seq: row.get::<_, i64>(1) as u64,
        doc_bytes: row.get::<_, i64>(2) as u64,
        records: row.get::<_, i64>(3) as u64,
    }
}

/// §5.1 step 5's seq, under the step-1 lock: `head_seq + 1` and the instant it was taken.
///
/// `at` is rendered by PostgreSQL in the wire's own spelling, in UTC, so the store needs no date
/// library and no time zone of its own: a hub that formatted it locally would answer a change
/// whose `at` moved with the server's `TimeZone`.
///
/// Caveat: `at` has millisecond resolution, because that is the precision §5.1's example spells.
/// Two changes in one millisecond carry the same `at`, which is why `at` is informational and the
/// order is `seq` (§5.1).
pub(crate) async fn bump_seq(client: &mut Client, ws: &str) -> Result<Stamp, StoreError> {
    if crate::breaks::on("sequence-seq") {
        return nextval_seq(client).await;
    }
    let row = client
        .query_one(
            "UPDATE workspaces SET head_seq = head_seq + 1 WHERE id = $1 \
             RETURNING head_seq, to_char(clock_timestamp() AT TIME ZONE 'UTC', \
             'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[&ws],
        )
        .await?;
    Ok(Stamp {
        seq: row.get::<_, i64>(0) as u64,
        at: row.get(1),
    })
}

/// The seq from a `SEQUENCE`, which §5.1 forbids and `sequence-seq` forces on.
///
/// The value is gone the moment it is drawn: a rollback undoes the row and not the draw, so every
/// refusal after this point leaves a hole in the stream. That hole is the control's whole point.
async fn nextval_seq(client: &mut Client) -> Result<Stamp, StoreError> {
    let _ = client
        .execute("CREATE SEQUENCE IF NOT EXISTS change_seq_seq", &[])
        .await;
    let row = client
        .query_one(
            "SELECT nextval('change_seq_seq'), \
             to_char(clock_timestamp() AT TIME ZONE 'UTC', \
             'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"')",
            &[],
        )
        .await?;
    Ok(Stamp {
        seq: row.get::<_, i64>(0) as u64,
        at: row.get(1),
    })
}

/// §5.1's last seam before `COMMIT`: the change rows and the idempotency row are written and the
/// transaction is still open.
///
/// The pause belongs after the idempotency row, so a peer that answers a duplicate key finds the
/// row already there and does not have to reason about an uncommitted insert.
///
/// Caveat: `sequence-seq` holds an ODD seq here and not an even one, which is what widens the
/// window a rolled-back draw leaves open. It is a widening and not the cause: the hole itself
/// comes from `nextval` not being transactional, and `hundred_writers_have_no_gap` sees that
/// without the wait, because the refused batch it rolls back draws its seq first either way.
pub(crate) async fn before_commit(seq: u64) {
    if crate::breaks::on("sequence-seq") && seq % 2 == 0 {
        return;
    }
    super::HOOKS.pause_before_commit(seq).await;
}

/// §5.1 step 8, on the connection that committed: read the flush position and raise the high-water.
///
/// The *flush* position and not the write position, because the flush position is what an OS crash
/// cannot lose, and step 1's `synchronous_commit` is what makes it cover this commit.
///
/// Caveat: a reader that raises the high-water on a connection other than the one that committed
/// can read a flush position from before the commit, and the detector will then read the next
/// connection's database as a restore. That is why this takes the client and not the store.
pub(crate) async fn watermark(
    client: &mut Client,
    detector: &Detector,
    head: (&str, u64, u64),
) -> Result<(), StoreError> {
    let row = client
        .query_one("SELECT pg_current_wal_flush_lsn()::text", &[])
        .await?;
    let lsn: String = row.get(0);
    let (ws, epoch, seq) = head;
    detector.commit_watermark(&lsn, ws, epoch, seq);
    Ok(())
}
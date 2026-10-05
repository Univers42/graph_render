//! Drawing an epoch, and the workspace list a bump or a test needs.
//!
//! The clock itself lives in SQL (`hub_next_epoch`, in `sql/0002_epoch.sql`), because the triggers
//! call it too and there must be exactly one producer of an epoch. This module is the Rust-side
//! door onto it.

use tokio_postgres::Client;

use crate::error::StoreError;

/// Draw the next epoch from the shared clock.
///
/// Microseconds since the Unix epoch, and the run-ahead is microseconds too: N calls inside one
/// microsecond leave the clock N microseconds above the wall clock. That is deliberate — it is
/// what keeps two hubs from drawing the same epoch after a restore.
pub async fn bump_now(client: &mut Client) -> Result<u64, StoreError> {
    let row = client.query_one("SELECT hub_next_epoch()", &[]).await?;
    Ok(row.get::<_, i64>(0) as u64)
}

/// A workspace's `(epoch, head_seq)`, or `None` when there is no such workspace.
pub async fn head_of(client: &mut Client, ws: &str) -> Result<Option<(u64, u64)>, StoreError> {
    let row = client
        .query_opt(
            "SELECT epoch, head_seq FROM workspaces WHERE id = $1",
            &[&ws],
        )
        .await?;
    Ok(row.map(|r| (r.get::<_, i64>(0) as u64, r.get::<_, i64>(1) as u64)))
}

/// Every workspace id, in id order.
pub async fn workspace_ids(client: &mut Client) -> Result<Vec<String>, StoreError> {
    let rows = client
        .query("SELECT id FROM workspaces ORDER BY id", &[])
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(row.get(0));
    }
    Ok(out)
}
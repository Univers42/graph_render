//! §4's workspace create: one explicit transaction, no seq, and a `doc_bytes` that is already the
//! document's length.
//!
//! The shape is §5.1's with three differences, all of them the spec's: `INSERT … ON CONFLICT (id)
//! DO NOTHING` instead of an update, no change log, and the document's frame seeded rather than
//! accumulated.

use graph_contract::hub::{Limits, check_workspace_id};
use tokio_postgres::Client;

use crate::error::StoreError;
use crate::store::Store;
use crate::writer::bytes;
use crate::writer::retry::retried;
use crate::writer::step;

/// Create `ws`, or answer that it already existed.
///
/// The id is checked against the slug grammar **before** the connection opens: the `CHECK` on the
/// column would catch it, but a refusal that arrives as a driver error is a 500-class answer, and
/// this one is a 422.
pub(crate) async fn create(store: &Store, ws: &str, _limits: &Limits) -> Result<bool, StoreError> {
    check_workspace_id(ws)?;
    let mut client = store.client().await?;
    let (inserted, epoch) = retried!(store, client, once(&mut client, ws).await)?;
    step::watermark(&mut client, store.detector(), (ws, epoch, 0)).await?;
    Ok(inserted)
}

/// Why `create` takes the request's [`Limits`] and does not read it.
///
/// The caps §6 fixes for a write are all about a body, and a workspace create has none: the row is
/// an id, an epoch and a frame of about forty bytes. The parameter is part of the entry point the
/// plan fixes, and it is here so that a cap which *does* apply to a create later has a place to be
/// read from rather than a new signature.
async fn once(client: &mut Client, ws: &str) -> Result<(bool, u64), StoreError> {
    step::begin(client).await?;
    let seeded = bytes::frame(ws, 0, 0) as i64;
    let inserted = client
        .execute(
            "INSERT INTO workspaces (id, epoch, doc_bytes) \
             VALUES ($1, hub_next_epoch(), $2) ON CONFLICT (id) DO NOTHING",
            &[&ws, &seeded],
        )
        .await?;
    let epoch = read_epoch(client, ws).await?;
    client.batch_execute("COMMIT").await?;
    Ok((inserted == 1, epoch))
}

/// The epoch the row holds after the insert, whether or not the insert happened.
///
/// Read rather than taken from `hub_next_epoch()`: on a conflict the insert drew an epoch nobody
/// stored, and the epoch this row has is the one every later answer quotes.
async fn read_epoch(client: &mut Client, ws: &str) -> Result<u64, StoreError> {
    let row = client
        .query_one("SELECT epoch FROM workspaces WHERE id = $1", &[&ws])
        .await?;
    Ok(row.get::<_, i64>(0) as u64)
}

//! The cursor rule (spec §5.3): what a workspace still keeps, read in one statement.

use graph_contract::hub::Cursor;
use tokio_postgres::Client;

use super::CursorState;
use crate::error::StoreError;

/// The range of cursors a workspace can serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bounds {
    /// The workspace's epoch.
    pub epoch: u64,
    /// Its `head_seq`.
    pub head_seq: u64,
    /// The lowest kept seq, or `head_seq + 1` when the log keeps none.
    pub low: u64,
}

impl Bounds {
    /// The bounds of `ws`, or `NotFound` when there is no such workspace.
    ///
    /// One statement, so the epoch, the head and the lowest kept seq are read from one snapshot even
    /// outside a transaction.
    pub(crate) async fn read(client: &Client, ws: &str) -> Result<Bounds, StoreError> {
        let row = client
            .query_opt(
                "SELECT w.epoch, w.head_seq, \
                 COALESCE((SELECT min(h.seq) FROM change_headers h WHERE h.ws = w.id), \
                          w.head_seq + 1) \
                 FROM workspaces w WHERE w.id = $1",
                &[&ws],
            )
            .await?
            .ok_or_else(|| StoreError::NotFound {
                what: format!("workspace `{ws}`"),
            })?;
        Ok(Bounds {
            epoch: row.get::<_, i64>(0) as u64,
            head_seq: row.get::<_, i64>(1) as u64,
            low: row.get::<_, i64>(2) as u64,
        })
    }

    /// Whether `since` is servable against these bounds.
    pub(crate) fn state(&self, since: Cursor) -> CursorState {
        let in_range = since.seq.saturating_add(1) >= self.low && since.seq <= self.head_seq;
        if since.epoch == self.epoch && in_range {
            CursorState::Valid
        } else {
            CursorState::Gone
        }
    }
}

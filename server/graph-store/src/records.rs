//! A plugin's records, one keyset page at a time (spec §5.3, Task 8).
//!
//! The page and the plugin's own position are read in one `REPEATABLE READ, READ ONLY` snapshot, so
//! `plugin_seq` names the change that the rows already include. Keys compare in byte order
//! (`COLLATE "C"`), the order the wire sorts ids in, never the database's locale.

use graph_contract::hub::Cursor;
use tokio_postgres::Client;

use crate::error::StoreError;
use crate::store::Store;

/// One records request, already validated by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordsReq {
    /// The workspace.
    pub ws: String,
    /// The plugin whose records are listed.
    pub plugin: String,
    /// The `(qcoll, id)` the previous page ended on, or `None` for the first page.
    pub after: Option<(String, String)>,
    /// The most rows one page carries.
    pub limit: u64,
}

/// One page of a plugin's records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordsPage {
    /// The workspace's epoch and the seq of the plugin's last change.
    pub plugin_seq: Cursor,
    /// `(qcoll, id, rev)` in byte order of `(qcoll, id)`.
    pub rows: Vec<(String, String, u64)>,
    /// The key to ask after next, or `None` when this page is the last.
    pub next: Option<(String, String)>,
}

/// One page of `req.plugin`'s records in `req.ws`.
///
/// `NotFound` for a missing workspace or a plugin with no manifest there.
pub async fn page(store: &Store, req: &RecordsReq) -> Result<RecordsPage, StoreError> {
    let client = store.client().await?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .await?;
    let plugin_seq = position(&client, &req.ws, &req.plugin).await?;
    let rows = rows(&client, req).await?;
    client.batch_execute("COMMIT").await?;
    let full = u64::try_from(rows.len()).is_ok_and(|n| n == req.limit);
    let next = full
        .then(|| rows.last().map(|(q, id, _)| (q.clone(), id.clone())))
        .flatten();
    Ok(RecordsPage {
        plugin_seq,
        rows,
        next,
    })
}

/// The workspace's epoch and the plugin's `plugin_seq`.
async fn position(client: &Client, ws: &str, plugin: &str) -> Result<Cursor, StoreError> {
    let row = client
        .query_opt(
            "SELECT w.epoch, m.plugin_seq FROM workspaces w \
             LEFT JOIN manifests m ON m.ws = w.id AND m.plugin = $2 WHERE w.id = $1",
            &[&ws, &plugin],
        )
        .await?
        .ok_or_else(|| StoreError::NotFound {
            what: format!("workspace `{ws}`"),
        })?;
    let seq: Option<i64> = row.get(1);
    let seq = seq.ok_or_else(|| StoreError::NotFound {
        what: format!("plugin `{plugin}` of `{ws}`"),
    })?;
    Ok(Cursor {
        epoch: row.get::<_, i64>(0) as u64,
        seq: seq as u64,
    })
}

/// The rows after `req.after`, at most `req.limit`.
async fn rows(client: &Client, req: &RecordsReq) -> Result<Vec<(String, String, u64)>, StoreError> {
    let (qcoll, id) = req.after.clone().unwrap_or_default();
    let rows = client
        .query(
            "SELECT qcoll, id, rev FROM records WHERE ws = $1 AND plugin = $2 \
             AND (qcoll COLLATE \"C\", id COLLATE \"C\") > ($3::text COLLATE \"C\", $4::text COLLATE \"C\") \
             ORDER BY qcoll COLLATE \"C\", id COLLATE \"C\" LIMIT $5",
            &[
                &req.ws,
                &req.plugin,
                &qcoll,
                &id,
                &i64::try_from(req.limit).unwrap_or(i64::MAX),
            ],
        )
        .await?;
    Ok(rows
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get::<_, i64>(2) as u64))
        .collect())
}

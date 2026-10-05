//! One page of the feed: the cursor check, the headers and their operations, all in one snapshot.

use std::collections::BTreeMap;

use graph_contract::hub::{ChangeHead, Cursor, Manifest, change_json, manifest_change_json};
use tokio_postgres::{Client, Row};

use super::cursor::Bounds;
use super::{Change, ChangeKind, ChangeOp, ChangePage, ChangesReq, CursorState, HOOKS};
use crate::error::{DbError, StoreError};
use crate::materialize::record;
use crate::store::Store;

/// One stored header.
struct Header {
    seq: u64,
    plugin: String,
    at: String,
    kind: ChangeKind,
    bytes: u64,
    ops: usize,
}

/// The manifests of the page's workspace, read the first time a manifest change needs them.
type Manifests = Option<BTreeMap<String, Manifest>>;

/// Read one page in its own `REPEATABLE READ, READ ONLY` transaction.
///
/// The `changes-read-committed` break reads it at `READ COMMITTED` instead, so a prune that commits
/// between the header and operation reads takes rows from under the page and the row turns red.
pub(crate) async fn read(store: &Store, req: &ChangesReq) -> Result<ChangePage, StoreError> {
    let client = store.client().await?;
    let isolation = if crate::breaks::on("changes-read-committed") {
        "READ COMMITTED"
    } else {
        "REPEATABLE READ"
    };
    client
        .batch_execute(&format!("BEGIN ISOLATION LEVEL {isolation}, READ ONLY"))
        .await?;
    let page = read_in(&client, req).await?;
    client.batch_execute("COMMIT").await?;
    Ok(page)
}

/// The page, inside the open transaction.
async fn read_in(client: &Client, req: &ChangesReq) -> Result<ChangePage, StoreError> {
    let bounds = Bounds::read(client, &req.ws).await?;
    if bounds.state(req.since) == CursorState::Gone {
        return Err(StoreError::Gone);
    }
    let headers = headers(client, req).await?;
    if let Some(last) = headers.last() {
        HOOKS.pause_after_headers(last.seq).await;
    }
    let mut ops = operations(client, &req.ws, &headers).await?;
    let mut manifests: Manifests = None;
    let mut changes = Vec::with_capacity(headers.len());
    for header in headers {
        let found = ops.remove(&header.seq).unwrap_or_default();
        changes.push(build(client, &req.ws, header, found, &mut manifests).await?);
    }
    let seq = changes.last().map_or(req.since.seq, |c| c.seq);
    Ok(ChangePage {
        epoch: bounds.epoch,
        head_seq: bounds.head_seq,
        next: Cursor {
            epoch: bounds.epoch,
            seq,
        },
        bytes: changes.iter().map(|c| c.text.len() as u64).sum(),
        changes,
    })
}

/// The headers after `req.since`: at most `req.limit`, cut where their stored `bytes` would pass
/// `req.max_bytes`, but never cut to nothing.
async fn headers(client: &Client, req: &ChangesReq) -> Result<Vec<Header>, StoreError> {
    let rows = client
        .query(
            "SELECT seq, plugin, \
             to_char(at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.MS\"Z\"'), \
             kind, bytes, ops \
             FROM change_headers WHERE ws = $1 AND seq > $2 ORDER BY seq LIMIT $3",
            &[
                &req.ws,
                &(req.since.seq as i64),
                &i64::try_from(req.limit).unwrap_or(i64::MAX),
            ],
        )
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    let mut total = 0u64;
    for row in &rows {
        let header = header_of(row);
        total = total.saturating_add(header.bytes);
        if !out.is_empty() && total > req.max_bytes {
            break;
        }
        out.push(header);
    }
    Ok(out)
}

/// One header row.
fn header_of(row: &Row) -> Header {
    let kind: &str = row.get(3);
    Header {
        seq: row.get::<_, i64>(0) as u64,
        plugin: row.get(1),
        at: row.get(2),
        kind: if kind == "manifest" {
            ChangeKind::Manifest
        } else {
            ChangeKind::Batch
        },
        bytes: row.get::<_, i64>(4) as u64,
        ops: row.get::<_, i32>(5) as usize,
    }
}

/// The operations of `headers`, grouped by seq, each as `(is_upsert, op)` in change order.
async fn operations(
    client: &Client,
    ws: &str,
    headers: &[Header],
) -> Result<BTreeMap<u64, Vec<(bool, ChangeOp)>>, StoreError> {
    let mut out: BTreeMap<u64, Vec<(bool, ChangeOp)>> = BTreeMap::new();
    if headers.is_empty() {
        return Ok(out);
    }
    let seqs: Vec<i64> = headers.iter().map(|h| h.seq as i64).collect();
    let rows = client
        .query(
            "SELECT seq, op, qcoll, id, rev, text FROM change_ops \
             WHERE ws = $1 AND seq = ANY($2) ORDER BY seq, ord",
            &[&ws, &seqs],
        )
        .await?;
    for row in &rows {
        let op = ChangeOp {
            qcoll: row.get(2),
            id: row.get(3),
            rev: row.get::<_, i64>(4) as u64,
            text: row.get(5),
        };
        let is_upsert = row.get::<_, &str>(1) == "upsert";
        let seq = row.get::<_, i64>(0) as u64;
        out.entry(seq).or_default().push((is_upsert, op));
    }
    Ok(out)
}

/// One change from its header and the operations found for it.
///
/// A count that differs from the header's is a store fault: the change is refused rather than sent
/// with operations missing.
async fn build(
    client: &Client,
    ws: &str,
    header: Header,
    found: Vec<(bool, ChangeOp)>,
    manifests: &mut Manifests,
) -> Result<Change, StoreError> {
    if found.len() != header.ops {
        return Err(fault(format!(
            "change {} has {} operations, header says {}",
            header.seq,
            found.len(),
            header.ops
        )));
    }
    let (ups, dels): (Vec<_>, Vec<_>) = found.into_iter().partition(|(up, _)| *up);
    let upserts: Vec<ChangeOp> = ups.into_iter().map(|(_, op)| op).collect();
    let deletes: Vec<ChangeOp> = dels.into_iter().map(|(_, op)| op).collect();
    let head = ChangeHead {
        seq: header.seq,
        plugin: &header.plugin,
        at: &header.at,
    };
    let text = match header.kind {
        ChangeKind::Batch => batch_text(&head, &upserts, &deletes)?,
        ChangeKind::Manifest => manifest_text(client, ws, &head, manifests).await?,
    };
    Ok(Change {
        seq: header.seq,
        plugin: header.plugin,
        at: header.at,
        kind: header.kind,
        upserts,
        deletes,
        text,
    })
}

/// A batch change's text, rebuilt from its stored operations by graph-contract.
fn batch_text(
    head: &ChangeHead<'_>,
    upserts: &[ChangeOp],
    deletes: &[ChangeOp],
) -> Result<String, StoreError> {
    let records = upserts
        .iter()
        .map(|op| Ok((record::parse(&op.text)?, op.rev)))
        .collect::<Result<Vec<_>, StoreError>>()?;
    let removed: Vec<(String, String, u64)> = deletes
        .iter()
        .map(|op| (op.qcoll.clone(), op.id.clone(), op.rev))
        .collect();
    Ok(change_json(head, &records, &removed))
}

/// A manifest change's text.
///
/// Caveat: the log keeps no copy of the manifest a change registered, so an older manifest change
/// is sent with the plugin's *current* manifest. v1 manifests only grow (spec §4), so the text a
/// client gets is a superset of the one it replaces, and it can be longer than the header's
/// `bytes`, which the page's byte cut reads. Keeping each version's text is the upgrade path.
async fn manifest_text(
    client: &Client,
    ws: &str,
    head: &ChangeHead<'_>,
    manifests: &mut Manifests,
) -> Result<String, StoreError> {
    if manifests.is_none() {
        *manifests = Some(crate::materialize::manifests(client, ws).await?);
    }
    let manifest = manifests
        .as_ref()
        .and_then(|all| all.get(head.plugin))
        .ok_or_else(|| {
            fault(format!(
                "change {} registers `{}`, which has no manifest",
                head.seq, head.plugin
            ))
        })?;
    Ok(manifest_change_json(head, manifest))
}

/// A fault in what the store itself wrote.
fn fault(message: String) -> StoreError {
    StoreError::Db(DbError::store("XX000", message))
}

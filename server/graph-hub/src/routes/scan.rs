//! Reading back what the store keeps, through the store's own public API only.
//!
//! graph-store exposes a change feed, a records page and a materialized document; it does **not**
//! expose "the manifest this plugin registered" or "this one record". Task 5 needs both — a batch
//! carries its manifest (§5.1) and `GET …/plugins` and `GET …/records/…` name them — so they are
//! read out of the change log here, which is a store API and not a store change.
//!
//! The log is the right source for both: a manifest change carries the plugin's manifest nested
//! whole (`manifest_change_json`), and a record's last operation carries its `rev` and its canonical
//! text. Scanning forward from the oldest cursor the workspace still keeps and keeping the **last**
//! match is therefore the current state, because a seq is never retracted and a `rev` only moves
//! forward.
//!
//! Caveat: the scan is `O(retained changes)` per call, where retention is §6's `RETAIN` (100 000 by
//! default), and a workspace past it cannot answer at all — the oldest manifest change may already
//! have been pruned. A store API for these two reads is a "decision needed"
//! (`docs/decisions/graph-hub.md`, round 2, Task 5); this module is what keeps every §5.2 status
//! reachable today.

use std::collections::BTreeMap;

use graph_contract::hub::{Cursor, Manifest, read_manifest};
use graph_store::changes::{Change, ChangeKind, ChangePage, ChangesReq};
use graph_store::{Store, StoreError};

/// How many seqs one doubling probe skips.
///
/// Caveat: a fixed 256 rather than a doubling, because the number of probes is what a caller waits
/// for and 256 keeps a full 100 000-change retention under 400 round trips while never being wrong
/// about the answer — a smaller step only costs more probes.
const PROBE_STEP: u64 = 256;

/// The changes one page of the walk carries. §6 gives `SSE_PAGE` (256) for exactly this shape.
const WALK_LIMIT: u64 = 256;

/// §6's `CHANGES_BYTES` for one page of the walk, so a page is bounded the way a `/changes` page is.
const WALK_BYTES: u64 = 8 << 20;

/// The oldest cursor `ws` can still be paged from.
///
/// A cursor below the retained low bound is `StoreError::Gone`, and the store publishes no accessor
/// for the low bound, so this starts at 0 and steps forward until a page comes back. The answer is a
/// cursor at or above the low bound, so paging from it returns every change that is still there.
pub async fn oldest(store: &Store, ws: &str, head: (u64, u64)) -> Result<Cursor, StoreError> {
    let mut seq = 0u64;
    loop {
        let at = seq.min(head.1);
        if servable(store, ws, head.0, at).await? {
            return Ok(Cursor {
                epoch: head.0,
                seq: at,
            });
        }
        // `head_seq` is always servable — it is at or above the low bound by definition — so this
        // arm is the loop's floor and the probe terminates even on a workspace that keeps nothing.
        if at >= head.1 {
            return Ok(Cursor {
                epoch: head.0,
                seq: head.1,
            });
        }
        seq = seq.saturating_add(PROBE_STEP);
    }
}

/// Is `Cursor { epoch, seq }` a cursor this workspace can be paged from?
async fn servable(store: &Store, ws: &str, epoch: u64, seq: u64) -> Result<bool, StoreError> {
    let request = ChangesReq {
        ws: ws.to_owned(),
        since: Cursor { epoch, seq },
        // One change per probe: the probe asks "is this cursor servable", not "give me the log".
        limit: 1,
        max_bytes: u64::MAX,
    };
    Ok(graph_store::changes::page(store, &request).await.is_ok())
}

/// Every retained change of `ws`, in seq order, folded through `fold`.
pub async fn fold_each<F>(
    store: &Store,
    ws: &str,
    head: (u64, u64),
    mut fold: F,
) -> Result<(), StoreError>
where
    F: FnMut(&Change),
{
    let mut cursor = oldest(store, ws, head).await?;
    while cursor.seq <= head.1 {
        let page = walk(store, ws, cursor).await?;
        if page.changes.is_empty() {
            return Ok(());
        }
        for change in &page.changes {
            fold(change);
        }
        cursor = page.next;
    }
    Ok(())
}

/// One page of the walk, cut where §6's byte cap cuts it.
async fn walk(store: &Store, ws: &str, since: Cursor) -> Result<ChangePage, StoreError> {
    let request = ChangesReq {
        ws: ws.to_owned(),
        since,
        limit: WALK_LIMIT,
        max_bytes: crate::config::capped(WALK_BYTES),
    };
    graph_store::changes::page(store, &request).await
}

/// Every plugin's manifest in `ws`, keyed by plugin id, read from the retained log.
pub async fn manifests(
    store: &Store,
    ws: &str,
    head: (u64, u64),
) -> Result<BTreeMap<String, Manifest>, StoreError> {
    let mut texts: BTreeMap<String, String> = BTreeMap::new();
    fold_each(store, ws, head, |change| {
        if change.kind == ChangeKind::Manifest {
            texts.insert(change.plugin.clone(), change.text.clone());
        }
    })
    .await?;
    let mut out = BTreeMap::new();
    for (plugin, text) in texts {
        if let Some(manifest) = manifest_of(&text, &plugin) {
            out.insert(plugin, manifest);
        }
    }
    Ok(out)
}

/// One plugin's manifest in `ws`, or `None` when it never registered one or its change is pruned.
pub async fn manifest(
    store: &Store,
    ws: &str,
    plugin: &str,
    head: (u64, u64),
) -> Result<Option<Manifest>, StoreError> {
    let mut text: Option<String> = None;
    fold_each(store, ws, head, |change| {
        if change.kind == ChangeKind::Manifest && change.plugin == plugin {
            text = Some(change.text.clone());
        }
    })
    .await?;
    Ok(text.as_deref().and_then(|body| manifest_of(body, plugin)))
}

/// One record as the store last wrote it: its `rev` and its own canonical text, or `None` when it
/// was deleted or never existed.
///
/// §5.1 admits a record at most once per batch across upserts and deletes, so reading a change's
/// upserts and then its deletes is the change's own order and no record can be both.
pub async fn record(
    store: &Store,
    ws: &str,
    qcoll: &str,
    id: &str,
    head: (u64, u64),
) -> Result<Option<(u64, String)>, StoreError> {
    let mut found: Option<(u64, String)> = None;
    fold_each(store, ws, head, |change| {
        for op in &change.upserts {
            if op.qcoll == qcoll && op.id == id {
                found = Some((op.rev, op.text.clone()));
            }
        }
        for op in &change.deletes {
            if op.qcoll == qcoll && op.id == id {
                found = None;
            }
        }
    })
    .await?;
    Ok(found)
}

/// The manifest nested in one manifest change's text, read by the contract's own reader.
///
/// `None` when the text is not a manifest change's, which is a store fault rather than a refusal:
/// the log's own builder wrote that text, so a `None` here means the log and the contract disagree.
fn manifest_of(text: &str, plugin: &str) -> Option<Manifest> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let nested = value.get("manifest")?;
    read_manifest(&nested.to_string(), plugin).ok()
}

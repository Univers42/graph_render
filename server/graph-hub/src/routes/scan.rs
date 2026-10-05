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
//! match is therefore the current state, because a seq is never retracted.
//!
//! Caveat: the scan is `O(retained changes)` per call, where retention is §6's `RETAIN` (100 000 by
//! default), and a workspace past it cannot answer at all — the oldest manifest change may have been
//! pruned. A store API for these two reads is a "decision needed" (`docs/decisions/graph-hub.md`,
//! round 2, Task 5); this module is the fallback that keeps every §5.2 status reachable today.

use std::collections::BTreeMap;

use graph_contract::hub::{Change, Cursor, Manifest, read_manifest};
use graph_store::{ChangeKind, Store, StoreError};

/// How many seqs one doubling probe skips. Caveat: exponential rather than a linear hunt, because
/// `RETAIN` is 100 000 and a linear hunt would be 100 000 round trips to find one cursor.
const PROBE_STEP: u64 = 1 << 8;

/// The oldest cursor `ws` can still be paged from, found by doubling probe.
///
/// A cursor below the retained low bound is `StoreError::Gone`, and the store publishes no accessor
/// for the low bound itself, so this starts at 0 and doubles until a page comes back. The answer is
/// a cursor at or above the low bound, so what follows it is every change that is still there.
pub async fn oldest(store: &Store, ws: &str, head: (u64, u64)) -> Result<Cursor, StoreError> {
    let mut seq = 0u64;
    loop {
        let candidate = Cursor {
            epoch: head.0,
            seq,
        };
        match probe(store, ws, head, candidate).await? {
            Ok(page) => return Ok(page.next),
            Err(_) => seq = seq.saturating_add(PROBE_STEP),
        }
    }
}

/// One page from `since`, or `Gone` for a cursor the workspace cannot serve.
async fn probe(
    store: &Store,
    ws: &str,
    head: (u64, u64),
    since: Cursor,
) -> Result<Result<graph_store::ChangePage, StoreError>, StoreError> {
    let request = graph_store::changes::ChangesReq {
        ws: ws.to_owned(),
        since,
        // One change per probe: the probe asks "is this cursor servable", not "give me the log".
        limit: 1,
        max_bytes: u64::MAX,
    };
    Ok(graph_store::changes::page(store, &request).await)
}

/// Every retained change of `ws`, in seq order, folded through `fold`.
///
/// `max_bytes` is §6's `CHANGES_BYTES` per page and `limit` is the page cap, so one page is bounded
/// the way a `/changes` page is; the loop is what walks the whole retained log.
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
        let page = page(store, ws, cursor, head.1).await?;
        let mut moved = false;
        for change in &page.changes {
            fold(change);
            moved = true;
        }
        cursor = page.next;
        if !moved {
            break;
        }
    }
    Ok(())
}

/// One page of the walk, cut where §6's byte cap cuts it.
async fn page(
    store: &Store,
    ws: &str,
    since: Cursor,
    head_seq: u64,
) -> Result<graph_store::ChangePage, StoreError> {
    let request = graph_store::changes::ChangesReq {
        ws: ws.to_owned(),
        since,
        limit: 256,
        max_bytes: crate::config::capped(8 << 20),
    };
    let page = graph_store::changes::page(store, &request).await?;
    let _ = head_seq;
    Ok(page)
}

/// Every plugin's manifest in `ws`, keyed by plugin id, read from the retained log.
///
/// A manifest change's text is graph-contract's `manifest_change_json`, which nests the manifest
/// whole under `"manifest"`, so the manifest is lifted out as text and read by the contract's own
/// reader — the hub is never a second parser of a manifest.
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
/// A delete operation carries an empty `text`, and it is kept as the answer's "gone": the last
/// operation for an id is the current state, because a `rev` only ever moves forward.
pub async fn record(
    store: &Store,
    ws: &str,
    qcoll: &str,
    id: &str,
    head: (u64, u64),
) -> Result<Option<(u64, String)>, StoreError> {
    let mut found: Option<(u64, String)> = None;
    let mut deleted = false;
    fold_each(store, ws, head, |change| {
        for op in change.upserts.iter().chain(change.deletes.iter()) {
            if op.qcoll != qcoll || op.id != id {
                continue;
            }
            if change.deletes.iter().any(|gone| gone.id == op.id) && op.text.is_empty() {
                deleted = true;
                found = None;
            } else {
                deleted = false;
                found = Some((op.rev, op.text.clone()));
            }
        }
    })
    .await?;
    if deleted {
        return Ok(None);
    }
    Ok(found)
}

/// The manifest nested in one manifest change's text, read by the contract's own reader.
///
/// `None` when the text is not a manifest change's, which is a store fault rather than a refusal:
/// the log's own builder wrote that text.
fn manifest_of(text: &str, plugin: &str) -> Option<Manifest> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let nested = value.get("manifest")?;
    read_manifest(&nested.to_string(), plugin).ok()
}

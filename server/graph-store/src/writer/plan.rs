//! §5.1 step 4's reading half: what a batch will write, decided before anything is written.
//!
//! Every refusal this module can raise is raised here, before the first write: a batch is atomic
//! (§5.1), and the cheapest way to be atomic for a refusal is to never have started.

use std::collections::BTreeMap;

use graph_contract::hub::{Manifest, check_collection_id, qualify};
use graph_contract::ingest::{Record, record_piece};
use tokio_postgres::Client;

use crate::error::StoreError;
use crate::writer::BatchWrite;
use crate::writer::bytes;
use crate::writer::idempotency;
use crate::writer::links;
use crate::writer::manifest::Caps;
use crate::writer::step::Workspace;

/// What one planned batch will write, and the two totals the caps are checked against.
pub(crate) struct Plan {
    /// The upserts that change something, in batch order.
    pub upserts: Vec<Planned>,
    /// The deletes that remove something, in batch order, as `(qcoll, id, rev)`.
    pub deletes: Vec<(String, String, u64)>,
    /// The workspace's document length after this batch.
    pub doc_bytes: u64,
    /// The plugin's record bytes after this batch.
    pub plugin_bytes: u64,
}

/// One upsert that will be written.
pub(crate) struct Planned {
    /// Its qualified collection.
    pub qcoll: String,
    /// Its id within that collection.
    pub id: String,
    /// The `rev` it is given: the stored one plus one, or one.
    pub rev: u64,
    /// The record, for the change log.
    pub record: Record,
    /// Its canonical text, which is what is stored and what is hashed.
    pub text: String,
    /// The SHA-256 of `text`, which is the decision the no-op rests on.
    pub sha: [u8; 32],
    /// The `links` rows it contributes.
    pub links: Vec<(String, String, String)>,
}

/// What a stored record holds, for the three questions the plan asks of it.
struct Existing {
    /// The store's revision for it.
    rev: u64,
    /// Its stored text's SHA-256.
    sha: Vec<u8>,
    /// Its stored text's length.
    bytes: u64,
}

/// The plugin's stored row: `plugin_seq` for `If-Match`, `plugin_bytes` for the cap.
pub(crate) struct Plugin {
    /// The seq of the plugin's last change.
    pub seq: u64,
    /// The bytes the plugin's records hold.
    pub bytes: u64,
}

/// Read the plugin's row, or refuse: a batch for a plugin with no manifest is a 404.
pub(crate) async fn read_plugin(
    client: &mut Client,
    ws: &str,
    plugin: &str,
) -> Result<Plugin, StoreError> {
    let row = client
        .query_opt(
            "SELECT plugin_seq, plugin_bytes FROM manifests WHERE ws = $1 AND plugin = $2",
            &[&ws, &plugin],
        )
        .await?;
    row.map(|row| Plugin {
        seq: row.get::<_, i64>(0) as u64,
        bytes: row.get::<_, i64>(1) as u64,
    })
    .ok_or_else(|| StoreError::NotFound {
        what: format!("plugin `{plugin}` of `{ws}`"),
    })
}

/// §5.1 step 3: `If-Match` against `plugin_seq`.
///
/// A 412 and not a 409: they are different statuses and `StoreError::code()` is what a server maps
/// to exactly one, so the two cannot share a variant. The refusal names both cursors because the
/// fix a client makes is to re-read `plugin_seq` and resend.
pub(crate) fn if_match(req: &BatchWrite, plugin: &Plugin, epoch: u64) -> Result<(), StoreError> {
    let Some(want) = req.if_match else {
        return Ok(());
    };
    if want.epoch == epoch && want.seq == plugin.seq {
        return Ok(());
    }
    Err(StoreError::PreconditionFailed {
        what: format!(
            "If-Match `{want}`, the plugin is at `{epoch}.{}`",
            plugin.seq
        ),
    })
}

/// Read every record the batch names, and nothing else.
///
/// §5.1 step 4 reads only the batch's ids — at most `max_batch` of them — rather than the
/// workspace, because the decision it feeds is per record and a full scan would make every batch
/// cost the workspace's size.
async fn read_existing(
    client: &mut Client,
    req: &BatchWrite,
) -> Result<BTreeMap<(String, String), Existing>, StoreError> {
    let (qcolls, ids) = keys_of(req);
    if qcolls.is_empty() {
        return Ok(BTreeMap::new());
    }
    let rows = client
        .query(
            "SELECT qcoll, id, rev, text_sha256, text_bytes FROM records \
             WHERE ws = $1 AND qcoll = ANY($2) AND id = ANY($3)",
            &[&req.ws, &qcolls, &ids],
        )
        .await?;
    let mut out = BTreeMap::new();
    for row in &rows {
        out.insert(
            (row.get::<_, String>(0), row.get::<_, String>(1)),
            Existing {
                rev: row.get::<_, i64>(2) as u64,
                sha: row.get(3),
                bytes: row.get::<_, i64>(4) as u64,
            },
        );
    }
    Ok(out)
}

/// The `(qcoll, id)` pairs the batch names, de-duplicated, as the two arrays `ANY` takes.
///
/// De-duplicated and sorted because `ANY` over an array with repeats scans the duplicates twice and
/// `Vec` has no set: the batch's own reader refuses a record named twice, and this cannot be the
/// place that finds it twice.
fn keys_of(req: &BatchWrite) -> (Vec<String>, Vec<String>) {
    let named = req
        .batch
        .upserts
        .iter()
        .map(|up| (qualify(&req.plugin, &up.collection), up.id.clone()))
        .chain(
            req.batch
                .deletes
                .iter()
                .map(|del| (qualify(&req.plugin, &del.collection), del.id.clone())),
        );
    let mut keys: Vec<(String, String)> = named.collect();
    keys.sort();
    keys.dedup();
    let qcolls: Vec<String> = keys.iter().map(|(qcoll, _)| qcoll.clone()).collect();
    let ids: Vec<String> = keys.iter().map(|(_, id)| id.clone()).collect();
    (qcolls, ids)
}

/// Every upsert whose canonical text differs from what is stored, with its new `rev`.
///
/// §5.1 step 4's decision, and it is a SHA-256 comparison of the canonical text rather than a
/// comparison of the text: `record_piece` is the one producer of that text, so two clients that
/// wrote the same record byte-identically hash the same. `-0` against `0` and a reordered map both
/// change the bytes and are therefore changes (Review Focus 5); a byte-identical resend is not.
fn upserts_of(
    req: &BatchWrite,
    known: &BTreeMap<(String, String), Existing>,
) -> Result<Vec<Planned>, StoreError> {
    let mut out = Vec::new();
    for up in &req.batch.upserts {
        check_collection_id(&up.collection).map_err(StoreError::Hub)?;
        let qcoll = qualify(&req.plugin, &up.collection);
        let record = up.record(&req.plugin);
        let text = record_piece(&record);
        let sha = idempotency::sha256(text.as_bytes());
        let old = known.get(&(qcoll.clone(), up.id.clone()));
        if old.is_some_and(|old| old.sha == sha) {
            continue;
        }
        let links = links_of(&req.manifest, &up.collection, &record);
        out.push(Planned {
            qcoll,
            id: up.id.clone(),
            rev: old.map_or(1, |old| old.rev + 1),
            record,
            text,
            sha,
            links,
        });
    }
    Ok(out)
}

/// The deletes that remove something, as `(qcoll, id, rev)`.
///
/// Deleting an absent record is a no-op (§4), so it is not in the list, takes no `rev` and writes
/// no operation row: a change that said it removed something it did not would be a change a
/// reader replaying it could not apply.
fn deletes_of(
    req: &BatchWrite,
    known: &BTreeMap<(String, String), Existing>,
) -> Result<Vec<(String, String, u64)>, StoreError> {
    let mut out = Vec::new();
    for del in &req.batch.deletes {
        check_collection_id(&del.collection).map_err(StoreError::Hub)?;
        let qcoll = qualify(&req.plugin, &del.collection);
        if let Some(old) = known.get(&(qcoll.clone(), del.id.clone())) {
            out.push((qcoll, del.id.clone(), old.rev));
        }
    }
    Ok(out)
}

/// The `(document, plugin)` byte deltas, and how many rows appeared and disappeared.
///
/// Signed, and split into the two totals because they are checked against two different caps: the
/// document's against `GRAPH_HUB_MAX_DOC_BYTES`, the plugin's against
/// `GRAPH_HUB_MAX_PLUGIN_BYTES` (§4). An update's own text replaces the stored one, so it adds its
/// length and subtracts the stored length.
fn deltas(
    upserts: &[Planned],
    deletes: &[(String, String, u64)],
    known: &BTreeMap<(String, String), Existing>,
) -> (i64, i64, u64, u64) {
    let mut doc = 0i64;
    let mut plugin = 0i64;
    let mut added = 0u64;
    for up in upserts {
        doc += up.text.len() as i64;
        plugin += up.text.len() as i64;
        if let Some(old) = known.get(&(up.qcoll.clone(), up.id.clone())) {
            doc -= old.bytes as i64;
            plugin -= old.bytes as i64;
        } else {
            added += 1;
        }
    }
    for (qcoll, id, _) in deletes {
        if let Some(old) = known.get(&(qcoll.clone(), id.clone())) {
            doc -= old.bytes as i64;
            plugin -= old.bytes as i64;
        }
    }
    (doc, plugin, added, deletes.len() as u64)
}

/// §5.1 step 4: the plan, its two byte totals, and its two caps.
///
/// The cap check comes before the caller writes anything, so a 413 refuses a batch that stored
/// nothing. `doc_bytes` moves by the frame delta and by the changed records' own lengths;
/// `plugin_bytes` moves by the same lengths for this plugin alone.
pub(crate) async fn plan(
    client: &mut Client,
    req: &BatchWrite,
    plugin: &Plugin,
    ws: &Workspace,
    caps: &Caps,
) -> Result<Plan, StoreError> {
    let known = read_existing(client, req).await?;
    let upserts = upserts_of(req, &known)?;
    let deletes = deletes_of(req, &known)?;
    let (doc, plugin_delta, added, removed) = deltas(&upserts, &deletes, &known);
    let frame = bytes::frame_delta(&req.ws, (0, ws.records), (0, ws.records + added - removed));
    let doc_bytes = bytes::add(ws.doc_bytes, doc + frame)?;
    let plugin_bytes = bytes::add(plugin.bytes, plugin_delta)?;
    bytes::cap("document", doc_bytes, caps.doc_bytes)?;
    bytes::cap("plugin", plugin_bytes, caps.plugin_bytes)?;
    Ok(Plan {
        upserts,
        deletes,
        doc_bytes,
        plugin_bytes,
    })
}

/// The `links` rows this record contributes, or none when the manifest declares no such collection.
///
/// `Batch::check` has already refused an upsert naming an undeclared collection, so the `None` arm
/// is unreachable through the entry points. It is `unwrap_or_default` rather than a refusal because
/// this runs inside the transaction whose real work is the records, and a record whose links cannot
/// be derived would be stored with no links — which the materializer's anti-join would read as "no
/// reference", silently. The check upstream is what keeps that from happening, and the test that
/// proves it is `a_batch_with_one_bad_record_changes_nothing`.
fn links_of(
    manifest: &Manifest,
    collection: &str,
    record: &Record,
) -> Vec<(String, String, String)> {
    manifest
        .collections
        .iter()
        .find(|c| c.id == collection)
        .map(|declared| links::tuples(record, declared))
        .unwrap_or_default()
}

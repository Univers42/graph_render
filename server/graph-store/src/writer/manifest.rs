//! §4's manifest PUT: growth, the two caps, and a change of kind `manifest`.
//!
//! The lock order is the batch's (§5.1 step 1, N9): `BEGIN`, the writer guard, the workspace row
//! `FOR UPDATE`. Nothing else is locked first, so a manifest PUT and a batch cannot deadlock.

use graph_contract::hub::{
    ChangeHead, Growth, MAX_PLUGINS, check_change, check_plugin_id, growth, manifest_change_json,
    manifest_json, read_manifest,
};
use tokio_postgres::Client;

use crate::error::StoreError;
use crate::store::Store;
use crate::writer::ManifestWrite;
use crate::writer::ManifestWritten;
use crate::writer::bytes;
use crate::writer::change::{Change, insert};
use crate::writer::retry::retried;
use crate::writer::step;

/// The two store-side caps a write checks, which are the store's and not the request's.
///
/// `graph_contract::hub::Limits` carries the three *wire* caps; `GRAPH_HUB_MAX_DOC_BYTES` and
/// `GRAPH_HUB_MAX_PLUGIN_BYTES` are §6's, they are per workspace and per plugin rather than per
/// request, and they come from the configuration the store was opened with.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Caps {
    /// `GRAPH_HUB_MAX_DOC_BYTES`.
    pub doc_bytes: u64,
    /// `GRAPH_HUB_MAX_PLUGIN_BYTES`.
    pub plugin_bytes: u64,
}

impl Caps {
    /// The caps `store` was opened with.
    pub(crate) fn of(store: &Store) -> Caps {
        Caps {
            doc_bytes: store.config().max_doc_bytes,
            plugin_bytes: store.config().max_plugin_bytes,
        }
    }
}

/// What one attempt answered, with the stamp step 8 needs.
struct Head {
    /// The answer the caller receives.
    written: ManifestWritten,
    /// The workspace's epoch under the step-1 lock.
    epoch: u64,
    /// The seq to hand the detector, which is the workspace's `head_seq` either way.
    seq: u64,
}

/// The manifest row as it is stored, read back for the growth comparison.
struct Stored {
    /// The canonical text the store wrote, which `growth` compares the parsed form of.
    text: String,
    /// How many collections it declares, for the frame delta.
    collections: u64,
    /// The bytes its declarations added to `doc_bytes`, which a growth replaces.
    decl_bytes: u64,
}

/// Register or grow `req.plugin`'s manifest in `req.ws`.
pub(crate) async fn put(store: &Store, req: &ManifestWrite) -> Result<ManifestWritten, StoreError> {
    check_plugin_id(&req.plugin)?;
    let caps = Caps::of(store);
    let mut client = store.client().await?;
    let head = retried!(store, client, once(&mut client, req, &caps).await)?;
    step::watermark(
        &mut client,
        store.detector(),
        (&req.ws, head.epoch, head.seq),
    )
    .await?;
    Ok(head.written)
}

/// One attempt: the whole transaction, `BEGIN` to `COMMIT`.
async fn once(client: &mut Client, req: &ManifestWrite, caps: &Caps) -> Result<Head, StoreError> {
    step::begin(client).await?;
    let workspace = step::lock_workspace(client, &req.ws).await?;
    let old = read_stored(client, &req.ws, &req.plugin).await?;
    let status = if old.is_some() { 200 } else { 201 };
    let outcome = decide(client, req, old.as_ref()).await?;
    let head = Head {
        written: ManifestWritten {
            status,
            seq: workspace.head_seq,
            growth: outcome,
        },
        epoch: workspace.epoch,
        seq: workspace.head_seq,
    };
    if outcome == Growth::Same {
        // A byte-identical resend still took step 1's lock and still commits: the lock is what
        // makes the equality check true against a concurrent write, and a rollback would be an
        // answer about a transaction that did nothing either way.
        client.batch_execute("COMMIT").await?;
        return Ok(head);
    }
    let seq = grow(client, req, caps, workspace.doc_bytes, old.as_ref()).await?;
    Ok(Head {
        written: ManifestWritten {
            status,
            seq,
            growth: outcome,
        },
        epoch: workspace.epoch,
        seq,
    })
}

/// `Growth::Same` for a byte-identical resend, `Grown` for anything the store will store.
///
/// The plugin cap is checked here rather than after the insert, so a refused registration leaves
/// nothing behind and the refusal cannot depend on whether the manifest happened to be valid.
async fn decide(
    client: &mut Client,
    req: &ManifestWrite,
    old: Option<&Stored>,
) -> Result<Growth, StoreError> {
    let Some(stored) = old else {
        let count = client
            .query_one("SELECT count(*) FROM manifests WHERE ws = $1", &[&req.ws])
            .await?
            .get::<_, i64>(0);
        if count as u64 >= MAX_PLUGINS {
            return Err(StoreError::Hub(graph_contract::hub::HubError::TooLarge {
                what: "plugins",
                limit: MAX_PLUGINS,
            }));
        }
        return Ok(Growth::Grown);
    };
    growth(&read_manifest(&stored.text, &req.plugin)?, &req.manifest).map_err(StoreError::Hub)
}

/// Everything after a `Grown`: the document cap, the seq, the change and the row.
///
/// Returns the seq it took, which is what the answer quotes.
async fn grow(
    client: &mut Client,
    req: &ManifestWrite,
    caps: &Caps,
    doc_bytes: u64,
    old: Option<&Stored>,
) -> Result<u64, StoreError> {
    let doc_bytes = bytes::add(doc_bytes, growth_delta(client, req, old).await?)?;
    bytes::cap("document", doc_bytes, caps.doc_bytes)?;
    let stamp = step::bump_seq(client, &req.ws).await?;
    let head = ChangeHead {
        seq: stamp.seq,
        plugin: &req.plugin,
        at: &stamp.at,
    };
    let text = manifest_change_json(&head, &req.manifest);
    check_change(&text, &req.limits)?;
    insert(
        client,
        &Change {
            ws: req.ws.clone(),
            seq: stamp.seq,
            plugin: req.plugin.clone(),
            at: stamp.at.clone(),
            kind: "manifest",
            text,
            ops: Vec::new(),
        },
    )
    .await?;
    write_manifest(client, req, stamp.seq).await?;
    write_doc_bytes(client, &req.ws, doc_bytes).await?;
    step::before_commit(stamp.seq).await;
    client.batch_execute("COMMIT").await?;
    Ok(stamp.seq)
}

/// How much `doc_bytes` moves when `req` replaces `old`: the declarations, and the separators
/// between collections counted over the **whole workspace**.
///
/// The workspace total, not this plugin's own count: `frame_bytes` drops one separator only
/// for the first collection of the document, so a second plugin's first collection adds one
/// separator per declaration, where a per-plugin count would have dropped one of them.
async fn growth_delta(
    client: &mut Client,
    req: &ManifestWrite,
    old: Option<&Stored>,
) -> Result<i64, StoreError> {
    let others = client
        .query_one(
            "SELECT COALESCE(sum(json_array_length((text::json) -> 'collections')), 0)::bigint \
             FROM manifests WHERE ws = $1 AND plugin <> $2",
            &[&req.ws, &req.plugin],
        )
        .await?
        .get::<_, i64>(0) as u64;
    let from = (others + old.map_or(0, |stored| stored.collections), 0);
    let to = (others + req.manifest.collections.len() as u64, 0);
    let replaced = old.map_or(0, |stored| stored.decl_bytes as i64);
    let declared = bytes::decl(&req.plugin, &req.manifest) as i64;
    Ok(bytes::frame_delta(&req.ws, from, to) + declared - replaced)
}

/// The manifest row itself. `plugin_bytes` is the plugin's records and is left alone: a manifest's
/// own text is capped by `MAX_MANIFEST_BYTES` at the reader and by the document cap here.
async fn write_manifest(
    client: &mut Client,
    req: &ManifestWrite,
    seq: u64,
) -> Result<(), StoreError> {
    let text = manifest_json(&req.manifest);
    client
        .execute(
            "INSERT INTO manifests (ws, plugin, version, text, text_bytes, decl_bytes, plugin_seq) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (ws, plugin) DO UPDATE SET version = EXCLUDED.version, \
             text = EXCLUDED.text, text_bytes = EXCLUDED.text_bytes, \
             decl_bytes = EXCLUDED.decl_bytes, plugin_seq = EXCLUDED.plugin_seq",
            &[
                &req.ws,
                &req.plugin,
                &(req.manifest.version as i32),
                &text,
                &(text.len() as i64),
                &(bytes::decl(&req.plugin, &req.manifest) as i64),
                &(seq as i64),
            ],
        )
        .await?;
    Ok(())
}

/// The workspace's document length, which the caps were just checked against.
async fn write_doc_bytes(client: &mut Client, ws: &str, doc_bytes: u64) -> Result<(), StoreError> {
    client
        .execute(
            "UPDATE workspaces SET doc_bytes = $2 WHERE id = $1",
            &[&ws, &(doc_bytes as i64)],
        )
        .await?;
    Ok(())
}

/// The stored manifest, or `None` when the plugin has none.
async fn read_stored(
    client: &mut Client,
    ws: &str,
    plugin: &str,
) -> Result<Option<Stored>, StoreError> {
    let row = client
        .query_opt(
            "SELECT text, decl_bytes FROM manifests WHERE ws = $1 AND plugin = $2",
            &[&ws, &plugin],
        )
        .await?;
    Ok(row.map(|row| {
        let text: String = row.get(0);
        let declared = read_manifest(&text, plugin).expect("a stored manifest reads back");
        Stored {
            collections: declared.collections.len() as u64,
            decl_bytes: row.get::<_, i64>(1) as u64,
            text,
        }
    }))
}

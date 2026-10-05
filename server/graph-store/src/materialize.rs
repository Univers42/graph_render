//! The streamed materializer (spec §5.4, Task 9): a workspace's document, written piece by piece
//! from one snapshot.
//!
//! `head() + Σ next() + tail()` is byte-identical to `graph_contract::hub::Model::to_json` over the
//! same records at the same cursor. The whole read runs in one `REPEATABLE READ, READ ONLY`
//! transaction, so a batch committed while the document streams is either wholly in it or wholly
//! out of it, and the cursor the document quotes is the snapshot's own `head_seq`.
//!
//! Records arrive in `(qcoll, id)` byte order through a keyset over the `COLLATE "C"` primary key,
//! `fetch_rows` at a time, so memory is bounded by one page and not by the workspace. Each row comes
//! with the references it holds that do not resolve (an anti-join on `links`), which is the only
//! fact pruning needs from outside the record.
//!
//! Caveat: the kept collections and the manifests are held in memory for the whole stream. That is
//! bounded by `MAX_PLUGINS × MAX_COLLECTIONS` declarations, not by the record count; a workspace at
//! both caps holds every declaration it would write in the head anyway.

mod declared;
mod document;
pub(crate) mod record;

use std::collections::BTreeMap;

use graph_contract::hub::{Cursor, Manifest, read_manifest};

use crate::error::StoreError;
use crate::store::Store;

pub use document::Document;

/// Open `ws`'s document at its current `head_seq`.
///
/// The transaction stays open until [`Document::next`] returns `None`; a `Document` dropped before
/// that drops its connection, and the server rolls the read back.
pub async fn open(store: &Store, ws: &str) -> Result<Document, StoreError> {
    let client = store.client().await?;
    client
        .batch_execute("BEGIN ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .await?;
    let row = client
        .query_opt(
            "SELECT epoch, head_seq, doc_bytes FROM workspaces WHERE id = $1",
            &[&ws],
        )
        .await?
        .ok_or_else(|| StoreError::NotFound {
            what: format!("workspace `{ws}`"),
        })?;
    let cursor = Cursor {
        epoch: row.get::<_, i64>(0) as u64,
        seq: row.get::<_, i64>(1) as u64,
    };
    let doc_bytes = row.get::<_, i64>(2) as u64;
    let manifests = manifests(&client, ws).await?;
    let declared = declared::Declared::of(&manifests);
    let opened = document::Opened {
        ws: ws.to_owned(),
        cursor,
        doc_bytes,
        fetch_rows: store.config().fetch_rows.max(1) as i64,
    };
    Ok(Document::new(client, opened, declared))
}

/// Every registered manifest of `ws`, by plugin, read back through graph-contract's own reader.
pub(crate) async fn manifests(
    client: &tokio_postgres::Client,
    ws: &str,
) -> Result<BTreeMap<String, Manifest>, StoreError> {
    let rows = client
        .query("SELECT plugin, text FROM manifests WHERE ws = $1", &[&ws])
        .await?;
    let mut out = BTreeMap::new();
    for row in rows {
        let plugin: String = row.get(0);
        let text: String = row.get(1);
        let manifest = read_manifest(&text, &plugin).map_err(StoreError::Hub)?;
        out.insert(plugin, manifest);
    }
    Ok(out)
}

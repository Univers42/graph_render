//! The writer transaction (spec §5.1, Task 7).
//!
//! # Scope of this file
//!
//! Everything the writer does to the database. The cases are split across `writer/` by concern so
//! no file is over the 300-line house limit; every test name is unchanged, so the rows still select
//! them (`--test writer idem` selects the three `idem_*` cases).
//!
//! The one thing these tests do not cover is the streamed document: `a_batch_with_one_bad_record_
//! changes_nothing` reads its evidence back through SQL here, and Task 9 reads it back through the
//! materializer, which is the stronger proof.
#![cfg(feature = "db-tests")]

#[path = "writer/batch.rs"]
mod batch;
#[path = "writer/bytes.rs"]
mod bytes;
#[path = "writer/deadlock.rs"]
mod deadlock;
#[path = "writer/idem.rs"]
mod idem;
#[path = "writer/if_match.rs"]
mod if_match;
#[path = "writer/manifest.rs"]
mod manifest;
#[path = "writer/sequence.rs"]
mod sequence;
#[path = "writer/space.rs"]
mod space;

mod support;

use graph_store::config::StoreConfig;
use graph_store::error::StoreError;
use graph_store::writer::{BatchWrite, Idempotency, ManifestWrite};
use graph_store::{BatchOutcome, Store};
use tokio_postgres::Client;

/// The manifest every case registers: one collection `task` with a title, a group, a parent and a
/// link to itself, which is what the record, the `links` rows and the byte totals need.
///
/// WHY one fixture rather than a manifest per case: the interesting refusals here are about the
/// *transaction*, and a per-case manifest would make two cases differ in two ways at once.
pub const MANIFEST: &str = r#"{
  "version": 1,
  "manifestVersion": 1,
  "name": "Tasks",
  "collections": [
    { "id": "task", "name": "Tasks", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null },
      { "id": "note", "name": "Note", "role": "scalar", "link": null },
      { "id": "state", "name": "State", "role": "group", "link": null },
      { "id": "up", "name": "Up", "role": "parent", "link": null }
    ] }
  ]
}"#;

/// A manifest with a second collection, for the growth case.
pub const GROWN: &str = r#"{
  "version": 1,
  "manifestVersion": 2,
  "name": "Tasks",
  "collections": [
    { "id": "task", "name": "Tasks", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null },
      { "id": "note", "name": "Note", "role": "scalar", "link": null },
      { "id": "state", "name": "State", "role": "group", "link": null },
      { "id": "up", "name": "Up", "role": "parent", "link": null }
    ] },
    { "id": "label", "name": "Label", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null }
    ] }
  ]
}"#;

/// The manifest a workspace with 64 plugins is past, read for a 65th.
pub const MANIFEST65: &str = r#"{
  "version": 1,
  "manifestVersion": 1,
  "name": "Sixty-fifth",
  "collections": [
    { "id": "task", "name": "Tasks", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null }
    ] }
  ]
}"#;

/// A store on `url`, with `config`'s limits layered over the defaults.
///
/// Each case needs its own database (`support::db::fresh_pair`), so the store is built per case from
/// the URL that came back rather than from the shared `GM_HUB_PG_URL`.
pub async fn store_on(url: &str, config: StoreConfig) -> Store {
    let mut cfg = config;
    cfg.url = url.to_owned();
    Store::connect(&cfg)
        .await
        .unwrap_or_else(|e| panic!("connect a store to {url}: {e}"))
}

/// A store with the defaults on `url`.
pub async fn store(url: &str) -> Store {
    store_on(url, StoreConfig::defaults()).await
}

/// A manifest read for `plugin`, through graph-contract's own reader.
///
/// WHY read the text rather than build the struct: a `Manifest` has no public constructor that
/// skips the qualification pass, and a case that built one by hand would store link targets the
/// reader never qualified — which is exactly the mistake the store must not be able to make.
pub fn manifest_of(text: &str, plugin: &str) -> graph_contract::hub::Manifest {
    graph_contract::hub::read_manifest(text, plugin)
        .unwrap_or_else(|e| panic!("the manifest reads: {e}"))
}

/// A manifest write for `plugin` at [`MANIFEST`].
pub fn manifest_write(ws: &str, plugin: &str) -> ManifestWrite {
    ManifestWrite {
        ws: ws.to_owned(),
        plugin: plugin.to_owned(),
        manifest: manifest_of(MANIFEST, plugin),
        limits: LIMITS,
    }
}

/// A manifest write carrying `text`, for the growth and conflict cases.
pub fn manifest_write_text(ws: &str, plugin: &str, text: &str) -> ManifestWrite {
    ManifestWrite {
        ws: ws.to_owned(),
        plugin: plugin.to_owned(),
        manifest: manifest_of(text, plugin),
        limits: LIMITS,
    }
}

/// The `Limits` every case reads a batch under.
pub const LIMITS: graph_contract::hub::Limits = graph_contract::hub::Limits::DEFAULT;

/// One batch: `upserts` as `(collection, id, updatedAt, cells)` and `deletes` as
/// `(collection, id)`, read through graph-contract's own reader so no case spells a body twice.
pub fn batch_of(
    upserts: &[(&str, &str, u32, &str)],
    deletes: &[(&str, &str)],
) -> graph_contract::hub::batch::Batch {
    let ups = upserts
        .iter()
        .map(|(c, id, at, cells)| {
            format!(r#"{{"collection":"{c}","id":"{id}","updatedAt":{at},"values":{{{cells}}}}}"#)
        })
        .collect::<Vec<_>>()
        .join(",");
    let dels = deletes
        .iter()
        .map(|(c, id)| format!(r#"{{"collection":"{c}","id":"{id}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    graph_contract::hub::batch::read_batch(
        &format!(r#"{{"upserts":[{ups}],"deletes":[{dels}]}}"#),
        &LIMITS,
    )
    .expect("the batch reads")
}

/// A batch write for `plugin`, with no key and no `If-Match`.
pub fn batch_write(ws: &str, plugin: &str, batch: graph_contract::hub::batch::Batch) -> BatchWrite {
    BatchWrite {
        ws: ws.to_owned(),
        plugin: plugin.to_owned(),
        manifest: manifest_of(MANIFEST, plugin),
        batch,
        idem: None,
        if_match: None,
        limits: LIMITS,
    }
}

/// A batch write carrying `key`, hashed over `body` as the hub hashes the raw bytes.
pub fn batch_write_with_key(
    ws: &str,
    plugin: &str,
    batch: graph_contract::hub::batch::Batch,
    key: &str,
    body: &[u8],
) -> BatchWrite {
    let mut req = batch_write(ws, plugin, batch);
    req.idem = Some(Idempotency {
        key: key.to_owned(),
        body_sha256: graph_store::writer::idempotency::sha256(body),
    });
    req
}

/// A workspace, a registered plugin, and a store — the state almost every case starts from.
pub async fn ready(name: &str) -> (Store, Client, String, String) {
    let (client, _, url) = support::db::fresh_pair(name).await;
    let store = store(&url).await;
    store
        .create_workspace("ws", &LIMITS)
        .await
        .expect("create the workspace");
    let written = store
        .put_manifest(&manifest_write("ws", "tracker"))
        .await
        .expect("register the manifest");
    assert_eq!(written.status, 201, "the first registration is a 201");
    (store, client, url, "ws".to_owned())
}

/// The `rev` stored for `id`, read straight out of `records`.
pub async fn rev_of(client: &mut Client, plugin: &str, id: &str) -> i64 {
    client
        .query_one(
            "SELECT rev FROM records WHERE ws = 'ws' AND qcoll = $1 AND id = $2",
            &[&format!("{plugin}.task"), &id],
        )
        .await
        .expect("read the stored rev")
        .get(0)
}

/// The text stored for `id`.
pub async fn text_of(client: &mut Client, plugin: &str, id: &str) -> String {
    client
        .query_one(
            "SELECT text FROM records WHERE ws = 'ws' AND qcoll = $1 AND id = $2",
            &[&format!("{plugin}.task"), &id],
        )
        .await
        .expect("read the stored text")
        .get(0)
}

/// Every seq in `change_headers`, in order.
pub async fn seqs(client: &mut Client) -> Vec<i64> {
    let rows = client
        .query(
            "SELECT seq FROM change_headers WHERE ws = 'ws' ORDER BY seq",
            &[],
        )
        .await
        .expect("read the change log");
    rows.iter().map(|row| row.get(0)).collect()
}

/// The workspace's `head_seq`.
pub async fn head_of(client: &mut Client) -> i64 {
    client
        .query_one("SELECT head_seq FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read head_seq")
        .get(0)
}

/// The `(seq, ord, op, id, rev)` of every change operation, in order.
pub async fn ops(client: &mut Client) -> Vec<(i64, i32, String, String, i64)> {
    let rows = client
        .query(
            "SELECT seq, ord, op, id, rev FROM change_ops WHERE ws = 'ws' ORDER BY seq, ord",
            &[],
        )
        .await
        .expect("read the change operations");
    rows.iter()
        .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4)))
        .collect()
}

/// The workspace's `doc_bytes`.
pub async fn doc_bytes(client: &mut Client) -> i64 {
    client
        .query_one("SELECT doc_bytes FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read doc_bytes")
        .get(0)
}

/// The status `error` answers with, through the one mapping a server uses.
pub fn status(error: &StoreError) -> &'static str {
    match error {
        StoreError::Hub(e) => match e.status() {
            422 => "422",
            413 => "413",
            409 => "409",
            _ => "hub-other",
        },
        StoreError::NotFound { .. } => "404",
        StoreError::PreconditionFailed { .. } => "412",
        StoreError::Duplicate { .. } => "409",
        other => {
            panic!(
                "the store refused with {other:?}, which maps to no class; a case that expected a \
                 status must say which one"
            )
        }
    }
}

/// The answer `outcome` carries, asserted in full so a change of the text's shape fails here.
pub fn assert_answer(outcome: &BatchOutcome, seq: u64, applied: u64) {
    assert_eq!(
        outcome.seq, seq,
        "the answer's seq (the workspace's `head_seq` when nothing applied)"
    );
    assert_eq!(outcome.applied, applied, "the answer's applied count");
    assert_eq!(
        outcome.response,
        format!(r#"{{"applied":{applied},"seq":{seq}}}"#),
        "the answer text is graph-contract's answer_json"
    );
}

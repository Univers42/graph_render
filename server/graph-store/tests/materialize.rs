//! The streamed materializer (spec §5.4, Task 9).
//!
//! Every case runs the same writes against the store and against graph-contract's in-memory
//! `Model`, then compares `head + Σ next + tail` with `Model::to_json` byte for byte. The prune
//! cases also assert on the document itself through `ingest::read`, because the `keep-dangling`
//! and `keep-cells` breaks turn the rule off in the Model too: an equality alone would stay green
//! under the negative control.
#![cfg(feature = "db-tests")]

#[path = "materialize/bytes.rs"]
mod bytes;
#[path = "materialize/equal.rs"]
mod equal;
#[path = "materialize/prune.rs"]
mod prune;
#[path = "materialize/random.rs"]
mod random;

mod support;

use std::collections::BTreeMap;

use graph_contract::hub::Model;
use graph_contract::hub::batch::Batch;
use graph_store::Store;
use graph_store::config::StoreConfig;
use graph_store::writer::{BatchWrite, ManifestWrite};
use tokio_postgres::Client;

/// Plugin `tracker`: a link out to `other.thing`, a link to itself, a parent, and two plain
/// collections, so every reference shape the pruner knows has a field to land on.
pub const TRACKER: &str = r#"{"version":1,"manifestVersion":1,"name":"Tasks","collections":[
 {"id":"task","name":"Tasks","titleField":"name","fields":[
  {"id":"name","name":"Name","role":"title","link":null},
  {"id":"state","name":"State","role":"group","link":null},
  {"id":"tags","name":"Tags","role":"tags","link":null},
  {"id":"link","name":"Link","role":"link","link":{"collection":"other.thing","cardinality":"one","symmetric":false}},
  {"id":"blocks","name":"Blocks","role":"link","link":{"collection":"task","cardinality":"many","symmetric":false}},
  {"id":"up","name":"Up","role":"parent","link":null},
  {"id":"note","name":"Note","role":"scalar","link":null}]},
 {"id":"c","name":"C","titleField":"name","fields":[{"id":"name","name":"Name","role":"title","link":null}]},
 {"id":"note","name":"Note","titleField":"name","fields":[{"id":"name","name":"Name","role":"title","link":null}]}]}"#;

/// Plugin `other`: the collection `tracker.task.link` points at.
pub const OTHER: &str = r#"{"version":1,"manifestVersion":1,"name":"Other","collections":[
 {"id":"thing","name":"Things","titleField":"name","fields":[
  {"id":"name","name":"Name","role":"title","link":null}]}]}"#;

/// The `Limits` every case reads a batch under.
pub const LIMITS: graph_contract::hub::Limits = graph_contract::hub::Limits::DEFAULT;

/// A manifest read for `plugin`, through graph-contract's own reader.
pub fn manifest_of(text: &str, plugin: &str) -> graph_contract::hub::Manifest {
    graph_contract::hub::read_manifest(text, plugin)
        .unwrap_or_else(|e| panic!("the manifest reads: {e}"))
}

/// One batch: `upserts` as `(collection, id, updatedAt, cells)` and `deletes` as
/// `(collection, id)`, read through graph-contract's own reader.
pub fn batch_of(upserts: &[(&str, &str, u32, &str)], deletes: &[(&str, &str)]) -> Batch {
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

/// The same workspace twice: in the store, and in graph-contract's `Model`.
pub struct Twin {
    /// The in-memory reference.
    pub model: Model,
    /// The store under test.
    pub store: Store,
    /// A plain connection to this case's database, for the SQL-side asserts.
    pub client: Client,
    /// The manifest text each plugin registered, for the batch writes.
    manifests: BTreeMap<String, String>,
}

impl Twin {
    /// A fresh database with workspace `ws`, read `fetch_rows` records per page.
    pub async fn new(name: &str, fetch_rows: u64) -> Twin {
        let (client, _, url) = support::db::fresh_pair(name).await;
        let mut cfg = StoreConfig::defaults();
        cfg.url = url.clone();
        cfg.fetch_rows = fetch_rows;
        let store = Store::connect(&cfg)
            .await
            .unwrap_or_else(|e| panic!("connect a store to {url}: {e}"));
        store
            .create_workspace("ws", &LIMITS)
            .await
            .expect("create the workspace");
        Twin {
            model: Model::new("ws").expect("the model"),
            store,
            client,
            manifests: BTreeMap::new(),
        }
    }

    /// Register `plugin`'s `text` on both sides.
    pub async fn register(&mut self, plugin: &str, text: &str) {
        self.model
            .register(plugin, manifest_of(text, plugin))
            .expect("the model registers");
        self.store
            .put_manifest(&ManifestWrite {
                ws: "ws".to_owned(),
                plugin: plugin.to_owned(),
                manifest: manifest_of(text, plugin),
                limits: LIMITS,
            })
            .await
            .expect("the store registers");
        self.manifests.insert(plugin.to_owned(), text.to_owned());
    }

    /// Apply `batch` for `plugin` on both sides; both must agree on whether it applies.
    pub async fn apply(&mut self, plugin: &str, batch: Batch) -> bool {
        let model = self.model.apply(plugin, &batch, &LIMITS).is_ok();
        let store = self
            .store
            .apply_batch(&BatchWrite {
                ws: "ws".to_owned(),
                plugin: plugin.to_owned(),
                manifest: manifest_of(&self.manifests[plugin], plugin),
                batch,
                idem: None,
                if_match: None,
                limits: LIMITS,
            })
            .await;
        assert_eq!(model, store.is_ok(), "model and store disagree: {store:?}");
        model
    }

    /// The whole streamed document.
    pub async fn read(&self) -> String {
        read_all(&self.store).await.0
    }
}

/// `ws`'s document as one string, with the document's own `doc_bytes` and cursor.
pub async fn read_all(store: &Store) -> (String, u64, graph_contract::hub::Cursor) {
    let mut doc = graph_store::materialize::open(store, "ws")
        .await
        .expect("open the document");
    let mut out = doc.head().to_owned();
    while let Some(piece) = doc.next().await.expect("the next piece") {
        out.push_str(&piece);
    }
    out.push_str(&doc.tail());
    (out, doc.doc_bytes(), doc.cursor())
}

/// The document read through the ingest reader, which is what a renderer will do with it.
pub fn ingest(doc: &str) -> graph_contract::ingest::Ingest {
    graph_contract::ingest::read(doc).unwrap_or_else(|e| panic!("the document reads: {e}"))
}

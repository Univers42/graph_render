//! A store with a workspace and one registered plugin: the state almost every store case starts
//! from.
//!
//! WHY in `support`: the writer cases and the change-feed cases both start here, and two copies of
//! `ready` would drift the first time one of them registered a different manifest.

use graph_store::Store;
use graph_store::config::StoreConfig;
use graph_store::writer::{BatchWrite, ManifestWrite};
use tokio_postgres::Client;

use super::fixture::{LIMITS, batch_of, manifest_of};

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

/// A manifest write for `plugin` at [`MANIFEST`].
pub fn manifest_write(ws: &str, plugin: &str) -> ManifestWrite {
    ManifestWrite {
        ws: ws.to_owned(),
        plugin: plugin.to_owned(),
        manifest: manifest_of(MANIFEST, plugin),
        limits: LIMITS,
    }
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

/// A workspace, a registered plugin, and a store — the state almost every case starts from.
pub async fn ready(name: &str) -> (Store, Client, String, String) {
    let (client, _, url) = super::db::fresh_pair(name).await;
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

/// The workspace's epoch.
pub async fn epoch_of(client: &Client) -> u64 {
    let epoch: i64 = client
        .query_one("SELECT epoch FROM workspaces WHERE id = 'ws'", &[])
        .await
        .expect("read the epoch")
        .get(0);
    epoch as u64
}

/// Run `sql` as the writer, so the epoch trigger does not count it as a foreign edit.
pub async fn as_writer(client: &Client, sql: &str) {
    client
        .batch_execute(&format!(
            "BEGIN; SELECT set_config('hub.writer', '1', true); {sql}; COMMIT"
        ))
        .await
        .unwrap_or_else(|e| panic!("run as the writer: {sql}: {e}"));
}

/// One batch per id in `ids`, each upserting task `id`: seqs 2, 3, … after the registration.
pub async fn write_tasks(store: &Store, ids: &[&str]) {
    for id in ids {
        let cells = format!(r#""name":"Task {id}""#);
        let outcome = store
            .apply_batch(&batch_write(
                "ws",
                "tracker",
                batch_of(&[("task", id, 1, &cells)], &[]),
            ))
            .await;
        assert!(outcome.is_ok(), "batch {id} applies: {outcome:?}");
    }
}

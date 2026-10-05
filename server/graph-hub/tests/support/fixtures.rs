//! The bodies the write cases send, and the hub a database case drives.
//!
//! WHY here: §4's wire shapes are graph-contract's, so a case that spelled one by hand would be a
//! second reader of the format. Every manifest and every batch below goes through graph-contract's
//! own writer, so a case cannot send something the hub's reader would refuse for a reason the case
//! did not mean.

use graph_contract::hub::{Manifest, manifest_json, read_manifest};

use super::{Hub, db};

/// The manifest most cases register: one collection `task` with a title, a scalar and a parent.
///
/// WHY one fixture: the refusals under test are about the transaction and the grant, and a per-case
/// manifest would make two cases differ in two ways at once.
pub const MANIFEST: &str = r#"{
  "version": 1,
  "manifestVersion": 1,
  "name": "Tasks",
  "collections": [
    { "id": "task", "name": "Tasks", "titleField": "name", "fields": [
      { "id": "name", "name": "Name", "role": "title", "link": null },
      { "id": "note", "name": "Note", "role": "scalar", "link": null },
      { "id": "up", "name": "Up", "role": "parent", "link": null }
    ] }
  ]
}"#;

/// A hub whose store reads `GM_HUB_PG_URL`, and `env` on top of the defaults.
pub fn hub_db(env: &[(&str, &str)]) -> Hub {
    let url = db::url();
    let mut all: Vec<(&str, &str)> = vec![("GRAPH_HUB_DB_URL", url.as_str())];
    all.extend_from_slice(env);
    super::hub_with_env(&all)
}

/// A hub over `grants`, whose store reads `GM_HUB_PG_URL`.
pub fn hub_db_grants(grants: &str) -> Hub {
    let url = db::url();
    super::hub_with_grants(grants, &[("GRAPH_HUB_DB_URL", url.as_str())])
}

/// The manifest at `version`, as graph-contract writes it back.
pub fn manifest_at(version: u32) -> String {
    manifest_of(MANIFEST, version)
}

/// A manifest with a second collection and `version`, which is growth and not a conflict.
pub fn grown() -> String {
    format!(
        r#"{{"version":1,"manifestVersion":{version},"name":"Tasks","collections":[
  {{ "id": "task", "name": "Tasks", "titleField": "name", "fields": [
    {{ "id": "name", "name": "Name", "role": "title", "link": null }},
    {{ "id": "note", "name": "Note", "role": "scalar", "link": null }},
    {{ "id": "up", "name": "Up", "role": "parent", "link": null }}
  ] }},
  {{ "id": "label", "name": "Label", "titleField": "name", "fields": [
    {{ "id": "name", "name": "Name", "role": "title", "link": null }}
  ] }}
]}}"#,
        version = version
    )
}

/// A manifest with the `note` field **removed**, at `version`: a shrink, which §4 refuses.
pub fn shrunk() -> String {
    format!(
        r#"{{"version":1,"manifestVersion":{version},"name":"Tasks","collections":[
  {{ "id": "task", "name": "Tasks", "titleField": "name", "fields": [
    {{ "id": "name", "name": "Name", "role": "title", "link": null }}
  ] }}
]}}"#,
        version = version
    )
}

/// A manifest named `Sixty-fifth` at `version`, for the plugin-count cap.
pub fn sixty_fifth(version: u32) -> String {
    format!(
        r#"{{"version":1,"manifestVersion":{version},"name":"Sixty-fifth","collections":[
  {{ "id": "task", "name": "Tasks", "titleField": "name", "fields": [
    {{ "id": "name", "name": "Name", "role": "title", "link": null }}
  ] }}
]}}"#,
        version = version
    )
}

/// `MANIFEST` at `version`, read through graph-contract's own reader and written back by its own
/// writer, so the body a case sends is the canonical spelling.
pub fn manifest_of(text: &str, version: u32) -> String {
    let source = text.replace("\"manifestVersion\": 1", &format!("\"manifestVersion\": {version}"));
    let parsed: Manifest = read_manifest(&source, "task")
        .unwrap_or_else(|error| panic!("the fixture manifest reads: {error}"));
    manifest_json(&parsed)
}

/// A batch body: `upserts` as `(collection, id, note)` and `deletes` as `(collection, id)`.
pub fn batch(upserts: &[(&str, &str, &str)], deletes: &[(&str, &str)]) -> String {
    let ups = upserts
        .iter()
        .map(|(collection, id, note)| {
            format!(
                r#"{{"collection":"{collection}","id":"{id}","updatedAt":1,"values":{{"name":"{id}","note":"{note}","up":null}}}}"#
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let dels = deletes
        .iter()
        .map(|(collection, id)| format!(r#"{{"collection":"{collection}","id":"{id}"}}"#))
        .collect::<Vec<_>>()
        .join(",");
    format!(r#"{{"version":1,"upserts":[{ups}],"deletes":[{dels}]}}"#)
}

/// One upsert of `id` into `collection`, with `note` as its scalar.
pub fn upsert(collection: &str, id: &str, note: &str) -> String {
    batch(&[(collection, id, note)], &[])
}

/// `PUT /v1/workspaces/{ws}` and `PUT .../plugins/{plugin}`, in that order: the state most write
/// cases start from.
///
/// Panics on a refusal: a fixture that could not reach the state a case needs has nothing to say.
pub async fn ready(hub: &Hub, ws: &str, plugin: &str) {
    let created = hub.put(&format!("/v1/workspaces/{ws}"), "").await;
    assert!(
        created.code() == 201 || created.code() == 200,
        "create {ws}: {} {}",
        created.code(),
        created.message()
    );
    let manifest = hub
        .put(
            &format!("/v1/workspaces/{ws}/plugins/{plugin}"),
            MANIFEST,
        )
        .await;
    assert!(
        manifest.code() == 201 || manifest.code() == 200,
        "register {plugin}: {} {}",
        manifest.code(),
        manifest.message()
    );
}

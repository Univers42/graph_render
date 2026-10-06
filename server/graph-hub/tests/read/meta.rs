//! `GET /v1/meta` and `GET /v1/workspaces`: the two rows §5.2 gives to "any key".
//!
//! They share a file because they share a fact: neither names a workspace, so neither can be
//! authorized against one, and the listing's filtering is the route's own work.

use crate::common::{names_of, ready_as};
use crate::support::add_key;
use crate::support::fixtures::{hub_db, hub_db_grants, ready};

/// `/v1/meta` spells out every §6 limit, so a client can size itself without reading the deploy doc.
#[tokio::test]
async fn meta_reports_every_limit_of_section_6() {
    let hub = hub_db(&[]).await;
    let reply = hub.get_with("/v1/meta").await;
    assert_eq!(reply.code(), 200, "{}", reply.body());
    let value: serde_json::Value = serde_json::from_str(&reply.body()).expect("a JSON answer");
    assert_eq!(value["api"], 1);
    assert!(value["version"].is_string(), "the hub's own version");
    let limits = value["limits"].as_object().expect("a limits object");
    for name in [
        "max_body",
        "max_batch",
        "max_record_bytes",
        "max_plugin_bytes",
        "max_doc_bytes",
        "retain",
        "retain_bytes",
        "changes_bytes",
        "body_timeout_ms",
        "stream_deadline_ms",
        "motor_timeout_ms",
        "timeout_ms",
        "sse_page",
        "fetch_rows",
        "last_seen",
        "writers",
        "readers",
        "layouts",
        "writers_per_key",
        "max_connections",
        "header_timeout_ms",
        "max_header_bytes",
        "max_subscribers",
        "max_subscribers_per_key",
    ] {
        assert!(limits.contains_key(name), "/v1/meta is missing {name}");
    }
    assert_eq!(limits["max_body"], 4 << 20, "§6's default");
    assert_eq!(limits["writers"], 2);
    assert_eq!(limits["sse_page"], 256);
}

/// The workspaces list holds only what the key may read, and each row carries its `epoch` and
/// `head_seq`.
#[tokio::test]
async fn the_workspaces_list_holds_only_what_the_key_may_read() {
    // `admin` per workspace, so each key can create the workspace it is granted and the listing
    // still has to filter: the grant names a workspace and nothing else differs between the keys.
    let hub = hub_db_grants("tester mine admin\nstranger theirs admin\n").await;
    let stranger = add_key(&hub, "stranger");
    hub.app.keys.reload().expect("both keys are in the pair");
    ready(&hub, "mine", "task").await;
    ready_as(&hub, &stranger, "theirs", "task").await;
    assert_eq!(
        names_of(&hub.get_with("/v1/workspaces").await),
        ["mine"],
        "only the granted workspace"
    );
    assert_eq!(
        names_of(&hub.get_as(&stranger, "/v1/workspaces").await),
        ["theirs"],
        "the other key sees only its own"
    );
}

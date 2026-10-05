//! The 403 matrix: a key is refused a workspace it may not touch **before** the store is asked
//! whether that workspace exists.
//!
//! The bytes of every refusal are identical, so a probe cannot tell a forbidden workspace from an
//! absent one — which is the property these cases are really about.

use axum::body::Body;

use crate::support::{KEY_NAME, hub_with_grants};

/// The two 403s are byte-identical, so a probe cannot tell a workspace it may not touch from one that
/// does not exist. This is the half that needs no database: authorization never asks.
#[tokio::test]
async fn refusal_bytes_are_identical_with_and_without_the_workspace() {
    // The grant names `ops` and the two requests name two workspaces that are not `ops`: the key may
    // not read either, and the bytes must be the same.
    let hub = hub_with_grants(&format!("{KEY_NAME} ops read\n"), &[]);
    let elsewhere = hub.get_with("/v1/workspaces/does-not-exist/graph").await;
    let also_elsewhere = hub.get_with("/v1/workspaces/never-created/graph").await;
    assert_eq!(elsewhere.code(), 403, "{}", elsewhere.body());
    assert_eq!(elsewhere.body(), also_elsewhere.body());
    // And the status line's own headers agree: one `Retry-After`-free 403 with the same JSON shape.
    assert_eq!(elsewhere.header("retry-after"), "");
    assert_eq!(elsewhere.header("www-authenticate"), "");
}

/// A key with no grant covering the path's workspace is 403 **before** any 404: the refusal never
/// reaches the store, so a workspace that is not there cannot change the answer.
#[tokio::test]
async fn no_grant_is_403_before_any_404() {
    let hub = hub_with_grants(&format!("{KEY_NAME} ops read\n"), &[]);
    for path in [
        "/v1/workspaces/nowhere/graph",
        "/v1/workspaces/nowhere/plugins/tracker/batches",
        "/v1/workspaces/nowhere/records/tracker/coll/1",
    ] {
        let reply = hub.get_with(path).await;
        assert_eq!(reply.code(), 403, "{path}: {}", reply.body());
        assert_eq!(reply.error(), "Forbidden", "{path}");
    }
}

/// A key granted `read` is refused a write with 403 and never 404, on every write route §5.2 names.
#[tokio::test]
async fn a_read_only_key_is_403_on_every_write_route() {
    let hub = hub_with_grants(&format!("{KEY_NAME} ops read\n"), &[]);
    let writes = [
        ("PUT", "/v1/workspaces/ops"),
        ("PUT", "/v1/workspaces/ops/plugins/tracker"),
        ("POST", "/v1/workspaces/ops/plugins/tracker/batches"),
    ];
    for (method, path) in writes {
        let reply = hub
            .send(hub.request(method, path).body(Body::from("{}")).unwrap())
            .await;
        assert_eq!(reply.code(), 403, "{method} {path}: {}", reply.body());
    }
}

/// A key granted `write:B` is refused A's records with 403 and never 404: it learns nothing about
/// another plugin's data, not even whether the workspace holds it.
#[tokio::test]
async fn a_key_learns_nothing_from_another_plugins_records() {
    let hub = hub_with_grants(&format!("{KEY_NAME} * write:b\n"), &[]);
    let mine = hub.get_with("/v1/workspaces/ops/plugins/b/records").await;
    let theirs = hub.get_with("/v1/workspaces/ops/plugins/a/records").await;
    assert_ne!(
        mine.code(),
        403,
        "its own plugin's records: {}",
        mine.body()
    );
    assert_eq!(
        theirs.code(),
        403,
        "another plugin's records: {}",
        theirs.body()
    );
    assert_eq!(theirs.error(), "Forbidden");
}

/// A key granted `admin` on one workspace learns nothing about another, which is the same 403 with
/// the same bytes.
#[tokio::test]
async fn admin_is_a_workspace_grant_and_not_a_hub_one() {
    let hub = hub_with_grants(&format!("{KEY_NAME} ops admin\n"), &[]);
    let mine = hub
        .send(
            hub.request("PUT", "/v1/workspaces/ops")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    let theirs = hub
        .send(
            hub.request("PUT", "/v1/workspaces/other")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await;
    assert_ne!(mine.code(), 403, "its own workspace: {}", mine.body());
    assert_eq!(theirs.code(), 403, "another workspace: {}", theirs.body());
}
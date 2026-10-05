//! `PUT .../plugins/{plugin}`: the manifest republication statuses and §4's two growth refusals,
//! plus §6's `MAX_PLUGINS` ceiling.

use crate::support::fixtures::{grown, hub_db, manifest_at, shrunk, sixty_fifth};

/// A manifest PUT is 201 then 200, and re-sending byte-identical content takes no seq: §4 says a
/// same-content republication is not growth.
#[tokio::test]
async fn put_manifest_is_201_then_200_and_the_same_content_takes_no_seq() {
    let hub = hub_db(&[]).await;
    assert_eq!(hub.put("/v1/workspaces/same", "").await.code(), 201);
    let first = hub
        .put("/v1/workspaces/same/plugins/task", manifest_at(1))
        .await;
    assert_eq!(first.code(), 201, "{}", first.body());
    let again = hub
        .put("/v1/workspaces/same/plugins/task", manifest_at(1))
        .await;
    assert_eq!(again.code(), 200, "{}", again.body());
    let third = hub
        .put("/v1/workspaces/same/plugins/task", manifest_at(1))
        .await;
    assert_eq!(third.code(), 200, "the third is still 200");
}

/// §4's shrink: a manifest that removes a field is a 409, never a silent deletion of stored
/// records.
#[tokio::test]
async fn put_manifest_refuses_a_removed_field_with_409() {
    let hub = hub_db(&[]).await;
    assert_eq!(hub.put("/v1/workspaces/shrink", "").await.code(), 201);
    let grown = hub.put("/v1/workspaces/shrink/plugins/task", grown()).await;
    assert_eq!(grown.code(), 201, "{}", grown.body());
    let shrunk = hub
        .put("/v1/workspaces/shrink/plugins/task", shrunk())
        .await;
    assert_eq!(shrunk.code(), 409, "{}", shrunk.body());
    assert_eq!(shrunk.error(), "conflict");
}

/// The same `manifestVersion` with other content is a 409 too: growth compares facts, and two
/// different manifests at one version are not a growth.
#[tokio::test]
async fn put_manifest_refuses_the_same_version_with_other_content_with_409() {
    let hub = hub_db(&[]).await;
    assert_eq!(hub.put("/v1/workspaces/stale", "").await.code(), 201);
    let first = hub
        .put("/v1/workspaces/stale/plugins/task", manifest_at(2))
        .await;
    assert_eq!(first.code(), 201, "{}", first.body());
    let other = hub.put("/v1/workspaces/stale/plugins/task", grown()).await;
    assert_eq!(other.code(), 409, "{}", other.body());
}

/// §6's `MAX_PLUGINS` is 64, so the sixty-fifth registration is a 413 and the first sixty-four stay.
#[tokio::test]
async fn put_manifest_refuses_the_sixty_fifth_plugin_with_413() {
    let hub = hub_db(&[]).await;
    assert_eq!(hub.put("/v1/workspaces/many", "").await.code(), 201);
    for index in 0..64 {
        let name = format!("p{index}");
        let reply = hub
            .put(
                &format!("/v1/workspaces/many/plugins/{name}"),
                sixty_fifth(1),
            )
            .await;
        assert_eq!(reply.code(), 201, "plugin {index}: {}", reply.body());
    }
    let refused = hub
        .put("/v1/workspaces/many/plugins/p64", sixty_fifth(1))
        .await;
    assert_eq!(refused.code(), 413, "{}", refused.body());
    assert_eq!(refused.error(), "too_large");
}
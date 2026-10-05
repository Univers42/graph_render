//! The two routes that need nothing: liveness, and the shape of every refusal.
#![cfg(not(feature = "db-tests"))]

mod support;

use support::*;

/// `/healthz` answers without a key, or the image's HEALTHCHECK cannot probe the hub.
#[tokio::test]
async fn healthz_needs_no_key() {
    let hub = hub_with_env(&[]);
    let reply = hub.get("/healthz").await;
    assert_eq!(reply.code(), 200);
    assert_eq!(reply.body(), "ok");
}

/// An unknown path is the JSON 404 shape, never axum's empty body, so a client can parse it.
#[tokio::test]
async fn an_unknown_route_is_the_json_404_shape() {
    let hub = hub_with_env(&[]);
    let reply = hub.get("/v1/nope").await;
    assert_eq!(reply.code(), 404);
    assert_eq!(reply.error(), "NotFound");
    assert!(!reply.message().is_empty());
}

/// The hub's own source never names the compute crate's `auth::check` (graph-render-4f's reuse
/// limit, `docs/decisions/graph-hub.md:120-121`): it carries the `any-key` break and the compute
/// `App`. A source grep is the only thing that can hold a negative.
#[test]
fn the_hub_source_never_names_auth_check() {
    let hits = support::grep_this_crate("auth::check");
    assert!(hits.is_empty(), "graph-hub must not call auth::check: {hits:?}");
}
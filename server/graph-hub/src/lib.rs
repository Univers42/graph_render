//! graph-hub: the hub's HTTP surface and nothing else. It owns the request, the credential, the
//! permit, the byte cap and the status; every byte of state lives in [`graph_store`] and every
//! byte of canonical text in `graph_contract`. It runs no motor math, writes no SQL and computes no
//! epoch.
//!
//! The router is §5.2's whole table: `/healthz` with no grant, the JSON 404 fallback so a client
//! never has to parse an empty body, and the twelve `/v1` routes under one authorization layer.
//! The layer is not an optimisation: it is what makes "authorization before existence" a property of
//! the router, since no handler can run before the grant has been decided.
//!
//! Task 1 wired `/healthz` and the fallback, Task 2 the settings and the start checks, Task 3 this
//! layer, and each later task adds its own handler to the same table.

pub mod app;
pub mod auth;
pub mod body;
pub mod breaks;
pub mod config;
pub mod error;
pub mod etag;
pub mod events;
pub mod gate;
pub mod grants;
pub mod health;
pub mod hooks;
pub mod keys;
pub mod observe;
pub mod relay;
pub mod routes;
pub mod serve;
pub mod watch;

use axum::Router;
use axum::routing::{get, post, put};
use std::sync::Arc;

pub use app::{App, LogSink};
pub use auth::Credential;
pub use error::{HubApiError, MotorFault};
pub use hooks::Hooks;

/// Every route. Anything else, a wrong method included, is a JSON 404, never axum's empty body.
///
/// The `/v1` routes are registered with their full paths and **not** nested, because the
/// authorization layer reads the request's own URI: `nest` rewrites it to the inner router's path
/// before a layer added outside the nest runs, so a nested `/v1` would reach the layer as
/// `/workspaces/ops/graph` and §5.2's table could not be matched against it.
///
/// Caveat: a flat table means the twelve paths are written out rather than mounted under a prefix, so
/// a path can only be reached under the prefix it was written with. That is the cost of reading the
/// URI in one place, and it is the one place that must read it.
pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/meta", get(routes::meta::meta))
        .route("/v1/workspaces", get(routes::workspaces::list))
        .route("/v1/workspaces/{ws}", put(routes::workspaces::put))
        .route("/v1/workspaces/{ws}/plugins", get(routes::plugins::list))
        .route(
            "/v1/workspaces/{ws}/plugins/{plugin}",
            put(routes::plugins::put),
        )
        .route(
            "/v1/workspaces/{ws}/plugins/{plugin}/batches",
            post(routes::batches::post),
        )
        .route(
            "/v1/workspaces/{ws}/plugins/{plugin}/records",
            get(routes::batches::page),
        )
        .route(
            "/v1/workspaces/{ws}/records/{plugin}/{collection}/{id}",
            get(routes::batches::one),
        )
        .route("/v1/workspaces/{ws}/graph", get(routes::graph::get))
        .route("/v1/workspaces/{ws}/changes", get(routes::changes::get))
        .route("/v1/workspaces/{ws}/events", get(events::get))
        .route("/v1/workspaces/{ws}/layout", post(relay::layout))
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .layer(axum::middleware::from_fn_with_state(
            Arc::clone(&app),
            auth::authorize_middleware,
        ))
        .with_state(app)
}

/// Liveness only: the process answers. No key, no grant, no permit and no database read, so the
/// image's `HEALTHCHECK` can probe a hub whose database is refusing it.
///
/// Caveat: `/healthz` is outside the authorization layer's reach on purpose — it is under `/` and not
/// `/v1` — so a credential cannot make the probe fail, and a key that has been rotated away cannot
/// take liveness with it.
async fn healthz() -> &'static str {
    "ok"
}

async fn not_found() -> HubApiError {
    HubApiError::NotFound(String::from("no such route"))
}

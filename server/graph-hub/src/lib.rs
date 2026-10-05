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
pub mod breaks;
pub mod config;
pub mod error;
pub mod grants;
pub mod health;
pub mod hooks;
pub mod keys;
pub mod observe;
pub mod serve;

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
        .route("/v1/meta", get(not_ready))
        .route("/v1/workspaces", get(not_ready).post(not_ready))
        .route("/v1/workspaces/{ws}", put(not_ready))
        .route("/v1/workspaces/{ws}/plugins", get(not_ready))
        .route("/v1/workspaces/{ws}/plugins/{plugin}", put(not_ready))
        .route(
            "/v1/workspaces/{ws}/plugins/{plugin}/batches",
            post(not_ready),
        )
        .route(
            "/v1/workspaces/{ws}/plugins/{plugin}/records",
            get(not_ready),
        )
        .route(
            "/v1/workspaces/{ws}/records/{plugin}/{collection}/{id}",
            get(not_ready),
        )
        .route("/v1/workspaces/{ws}/graph", get(not_ready))
        .route("/v1/workspaces/{ws}/changes", get(not_ready))
        .route("/v1/workspaces/{ws}/events", get(not_ready))
        .route("/v1/workspaces/{ws}/layout", post(not_ready))
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
    HubApiError::NotFound("no such route")
}

/// A route of §5.2's table whose handler its own task has not written yet.
///
/// The route exists so the router knows its path and its methods, which is what makes the
/// authorization layer's decision about the *right* grant and what turns a wrong method into a 405
/// rather than a 404. The handler is a 501 with the task that fills it, never a 200 and never a 404:
/// a 404 here would say the route does not exist, which is false.
async fn not_ready() -> HubApiError {
    HubApiError::NotImplemented
}

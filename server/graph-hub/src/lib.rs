//! graph-hub: the hub's HTTP surface and nothing else. It owns the request, the credential, the
//! permit, the byte cap and the status; every byte of state lives in [`graph_store`] and every
//! byte of canonical text in `graph_contract`. It runs no motor math, writes no SQL and computes
//! no epoch.
//!
//! The router is the whole of §5.2: twelve routes under `/v1`, `/healthz` with no grant, and a
//! JSON 404 fallback so a client never has to parse an empty body. Task 1 wires `/healthz` and the
//! fallback; each later task adds its own routes, so the table below grows one line at a time and
//! never names a handler that does not exist yet.

pub mod app;
pub mod breaks;
pub mod error;
pub mod health;
pub mod hooks;
pub mod observe;
pub mod serve;

use app::App;
use axum::Router;
use axum::routing::get;
use error::HubApiError;
use std::sync::Arc;

pub use app::{App, LogSink};
pub use error::{HubApiError, MotorFault};
pub use hooks::Hooks;

/// Every route. Anything else, a wrong method included, is a JSON 404, never axum's empty body.
pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .with_state(app)
}

/// Liveness only: the process answers. No key, no grant, no permit and no database read, so the
/// image's `HEALTHCHECK` can probe a hub whose database is refusing it.
async fn healthz() -> &'static str {
    "ok"
}

async fn not_found() -> HubApiError {
    HubApiError::NotFound("no such route")
}
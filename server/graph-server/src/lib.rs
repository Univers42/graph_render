//! graph-server: graph-motor as an HTTP microservice (`docs/contract/service-api.md`). It calls
//! the motor through `graph_wasm::service`, the same functions the browser build exports, so
//! the service and the embed cannot drift. The binary is `src/main.rs`; tests drive
//! [`router`] directly with `tower::ServiceExt::oneshot`.

pub mod app;
pub mod breaks;
pub mod config;
pub mod error;
pub mod health;
pub mod keys;
pub mod observe;
pub mod serve;

use app::App;
use axum::Router;
use axum::routing::get;
use error::ApiError;
use std::sync::Arc;

/// Every route. Anything else, a wrong method included, is a 404 `NotFound`.
pub fn router(app: Arc<App>) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .with_state(app)
}

/// Liveness only: the process answers.
async fn healthz() -> &'static str {
    "ok"
}

async fn not_found() -> ApiError {
    ApiError::not_found()
}

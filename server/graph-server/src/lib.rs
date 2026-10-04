//! graph-server: graph-motor as an HTTP microservice (`docs/contract/service-api.md`). It calls
//! the motor through `graph_wasm::service`, the same functions the browser build exports, so
//! the service and the embed cannot drift. The binary is `src/main.rs`; tests drive
//! [`router`] directly with `tower::ServiceExt::oneshot`.

pub mod app;
pub mod auth;
pub mod body;
pub mod breaks;
pub mod caps;
pub mod config;
pub mod embed;
pub mod error;
pub mod gate;
pub mod health;
pub mod keys;
pub mod layout;
pub mod meta;
pub mod motor;
pub mod observe;
pub mod query;
pub mod serve;

use app::App;
use axum::Router;
use axum::routing::{get, post};
use error::ApiError;
use std::sync::Arc;

/// Every route. Anything else, a wrong method included, is a 404 `NotFound`.
pub fn router(app: Arc<App>) -> Router {
    let observe = axum::middleware::from_fn_with_state(Arc::clone(&app), observe::observe);
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/meta", get(meta::meta))
        .route("/v1/layout", post(layout::layout))
        .route("/embed/{*path}", get(embed::serve))
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
        .layer(observe)
        .with_state(app)
}

/// Liveness only: the process answers.
async fn healthz() -> &'static str {
    "ok"
}

async fn not_found() -> ApiError {
    ApiError::not_found()
}

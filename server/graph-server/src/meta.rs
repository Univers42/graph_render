//! `GET /v1/meta`: what this server runs, ids in registry order. No `analyses`: the service
//! runs none (Verdict condition 12).

use crate::app::App;
use crate::error::ApiError;
use crate::observe::Facts;
use crate::{auth, motor, query};
use axum::extract::{Request, State};
use axum::http::header;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;

/// The API's own version, independent of the ABI's.
pub const API_VERSION: u32 = 1;

/// The handler.
pub async fn meta(State(app): State<Arc<App>>, request: Request) -> Response {
    let mut facts = Facts::default();
    let response = match describe(&app, &request, &mut facts) {
        Ok(body) => ([(header::CONTENT_TYPE, query::JSON_TYPE)], body).into_response(),
        Err(refused) => refused.into_response(),
    };
    facts.attach(response)
}

fn describe(app: &App, request: &Request, facts: &mut Facts) -> Result<String, ApiError> {
    let pairs = query::pairs(request.uri().query())?;
    facts.key = auth::check(app, request.headers())?;
    query::none(&pairs)?;
    let body = serde_json::json!({
        "api": API_VERSION,
        "abi": graph_wasm::ABI_VERSION,
        "version": env!("CARGO_PKG_VERSION"),
        "layouts": motor::layout_ids().collect::<Vec<_>>(),
        "posts": motor::post_ids().collect::<Vec<_>>(),
    });
    Ok(body.to_string())
}

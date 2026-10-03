//! The key check (Verdict condition 9): one `Authorization: Bearer <key>` header, the scheme
//! case-insensitive, the key held against every stored hash in constant time. A second
//! `Authorization` header is a 400; every other refusal is the same 401, whatever its cause.

use crate::app::App;
use crate::error::ApiError;
use axum::http::{HeaderMap, header};

/// The name of the key the request carries, `None` with auth off.
pub fn check(app: &App, headers: &HeaderMap) -> Result<Option<String>, ApiError> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let (first, second) = (values.next(), values.next());
    if second.is_some() {
        return Err(ApiError::bad_request("one Authorization header at most"));
    }
    let Some(store) = &app.keys else {
        return Ok(None);
    };
    let key = first
        .and_then(|value| value.to_str().ok())
        .and_then(bearer)
        .ok_or_else(ApiError::unauthorized)?;
    let keys = store.current();
    keys.name_of(key)
        .map(|name| Some(name.to_owned()))
        .ok_or_else(ApiError::unauthorized)
}

/// The token of a `Bearer` credential (RFC 6750 §2.1; the scheme is case-insensitive).
fn bearer(value: &str) -> Option<&str> {
    let (scheme, token) = value.trim().split_once(' ')?;
    let token = token.trim_start();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

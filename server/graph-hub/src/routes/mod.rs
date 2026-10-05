//! §5.2's twelve `/v1` routes, one module each, and the two functions every write shares.
//!
//! The order inside a write handler is fixed and is §5.2's order of refusals: **admit the permit
//! (or 503), then read the body (413/408), then call the store, then map its error.** Authorization
//! is not in the list because it is not in a handler: [`crate::auth::authorize_middleware`] decides
//! it before any of them runs, so a key with no grant is refused before it can learn that a gate is
//! full or that a workspace exists. That is the reason the permit is admitted here rather than in
//! the layer — see `docs/decisions/graph-hub.md` (round 2, Task 5).
//!
//! [`write_fault`] is the one place a [`StoreError`] becomes a status, so a store's own decision
//! (409, 412, 413, 422) keeps its status and the hub never re-derives the class. [`query_of`] is the
//! one place a query string is read, so two routes cannot disagree about what `?cursor=` means.

pub mod batches;
pub mod changes;
pub mod document;
pub mod graph;
pub mod meta;
pub mod plugins;
pub mod scan;
pub mod workspaces;

use axum::http::Uri;
use graph_contract::hub::{HubError, Limits};
use graph_store::StoreError;
use std::collections::BTreeMap;

use crate::app::App;
use crate::error::HubApiError;

/// The request's own query string, percent-decoded, first value per name.
///
/// WHY not `axum::extract::Query`: it is behind a feature the hub's edge deliberately does not
/// carry (plan fact 1), and this is the whole of the query reader the hub needs.
///
/// Caveat: a repeated parameter keeps the **first** value, because every parameter the hub reads
/// (`since`, `limit`, `cursor`) is read once per page and a caller that sent two sent a request the
/// page cannot honour. Taking the last would let a proxy's appended parameter override the client's.
pub(crate) fn query_of(uri: &Uri) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for pair in uri.query().unwrap_or("").split('&').filter(|p| !p.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let name = decode(name);
        if out.contains_key(&name) {
            continue;
        }
        out.insert(name, decode(value));
    }
    out
}

/// One percent-decoded query token; an undecodable one is left as it arrived, because the only
/// parameters the hub reads are then refused as malformed rather than silently dropped.
fn decode(text: &str) -> String {
    let replaced = text.replace('+', " ");
    percent_encoding::percent_decode_str(&replaced)
        .decode_utf8_lossy()
        .into_owned()
}

/// The contract's own `Limits`, built from §6's three numbers, for the readers that take one.
///
/// Caveat: this is a *copy* of `graph_contract::hub::Limits` on every call rather than a stored
/// value, so a handler cannot read a limit that the start check did not see. It is three `u64`s.
pub fn limits(app: &App) -> Limits {
    let read = &app.settings.limits;
    Limits {
        max_body: read.max_body,
        max_batch: read.max_batch,
        max_record_bytes: read.max_record_bytes,
    }
}

/// The store's `Limits`, for the readers that ask the store rather than the contract.
pub fn store_limits(app: &App) -> Limits {
    app.settings.store.limits()
}

/// One [`StoreError`] as the refusal §5.2 gives it.
///
/// The `Hub(HubError)` arm is the important one: `HubError::status()` already decided 409, 412, 413
/// or 422, and this keeps that status rather than the hub's guess.
pub fn write_fault(error: &StoreError) -> HubApiError {
    match error {
        StoreError::Hub(hub) => hub_fault(hub),
        // A wait that ran out is a 503 with `Retry-After: 1`, whichever wait it was: §5.1's one
        // retry has already happened inside the store by the time this is called.
        StoreError::Busy { .. } | StoreError::Serialization { .. } | StoreError::NoDatabase => {
            HubApiError::busy_wait()
        }
        StoreError::NotFound { what } => HubApiError::NotFound(leak(what)),
        StoreError::PreconditionFailed { .. } => HubApiError::PreconditionFailed,
        StoreError::Duplicate { what } => HubApiError::Conflict(leak(what)),
        StoreError::Gone => HubApiError::Gone(String::from("the cursor is outside what is kept")),
        // `Eof` and `Db` are hub defects: neither is a thing a caller can fix, and both are logged
        // whole by the handler that got them.
        StoreError::Db(_) | StoreError::Eof => HubApiError::Internal(error.code().to_owned()),
    }
}

/// One [`HubError`] as the refusal §5.2 gives it, with the status `HubError::status()` decided.
///
/// The default arm is 422 because `HubError::status()` says 422 for every variant that is not
/// `TooLarge`, `Conflict` or `Cursor`; the message is `HubError`'s own `Display`, so the coordinate
/// and the reason are the contract's text and not a paraphrase of it.
pub fn hub_fault(error: &HubError) -> HubApiError {
    match error {
        HubError::TooLarge { what, limit } => HubApiError::TooLarge {
            what,
            limit: *limit,
        },
        HubError::Conflict { what } => HubApiError::Conflict(what.clone()),
        HubError::Cursor { .. } => HubApiError::BadRequest("a cursor is <epoch>.<seq>"),
        _ => HubApiError::Invalid {
            path: String::new(),
            what: error.to_string(),
        },
    }
}

/// The message of a store refusal, which quotes only a name the caller itself sent.
fn leak(what: &str) -> String {
    if what.is_empty() {
        String::from("the store refused")
    } else {
        what.to_owned()
    }
}

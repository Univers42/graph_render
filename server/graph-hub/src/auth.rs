//! The request's credential and the authorization decision, and nothing else.
//!
//! Two functions: [`credential`] turns the `Authorization` header into the key's **name** (or a
//! refusal), and [`authorize`] adds the grant lookup and the path ids. Both live here so a handler
//! has one call to make and no way to skip half the decision.
//!
//! **Order is the whole point** (§5.2): authorization is decided before existence. No key or an
//! unknown key is 401; a key with no grant covering the path's workspace and plugin is 403; only
//! then may a missing workspace, plugin or record be 404. A key therefore learns that a workspace
//! exists only if it may read or write it, and the two 403s are byte-identical whichever workspace
//! was named — `no_grant_is_403_before_any_404` and
//! `refusal_bytes_are_identical_with_and_without_the_workspace` pin exactly that.

pub mod grant;
pub mod path;

pub use grant::Need;
pub use path::Ids;

use std::sync::Arc;

use axum::http::HeaderMap;
use axum::http::header;

use crate::app::App;
use crate::error::HubApiError;
use crate::keys::Pair;

/// Who a request is, once the credential and the grant both hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    /// The key's name, never its hash and never the key itself.
    pub key: String,
    /// The workspace the path names.
    pub ws: String,
    /// The plugin the path names, when it names one.
    pub plugin: Option<String>,
}

/// The name of the key the request carries, or a refusal.
///
/// Zero `Authorization` headers is 401 and more than one is **400**: RFC 9110 makes a message with
/// two of the same field invalid, and §5.2 requires exactly this, which is the one behaviour the
/// hub does that graph-server's own credential check does not
/// (`docs/decisions/graph-hub.md:120-121`).
///
/// Every other refusal of a credential is the same 401, so a caller cannot tell an unknown key
/// from a malformed one.
pub fn credential(pair: &Arc<Pair>, headers: &HeaderMap) -> Result<String, HubApiError> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let Some(first) = values.next() else {
        return Err(HubApiError::Unauthorized("missing or unknown API key"));
    };
    if values.next().is_some() {
        return Err(HubApiError::BadRequest("one Authorization header at most"));
    }
    let value = first
        .to_str()
        .map_err(|_| HubApiError::Unauthorized("missing or unknown API key"))?;
    let token =
        App::credential_of(value).ok_or(HubApiError::Unauthorized("missing or unknown API key"))?;
    pair.0
        .name_of(token)
        .map(str::to_owned)
        .ok_or(HubApiError::Unauthorized("missing or unknown API key"))
}

/// The credential and the grant, or the first refusal in §5.2's order.
///
/// `need` is what the route requires, and `ids` is the path's checked ids. The grant lookup is on
/// the key's **name**: `KeySet` has no enumerable key list (`entries` is private,
/// `server/graph-server/src/keys.rs:60`), so a key with no grant is denied without the hub ever
/// learning that the key exists.
pub fn authorize(
    app: &Arc<App>,
    headers: &HeaderMap,
    ids: &Ids,
    need: Need<'_>,
) -> Result<Credential, HubApiError> {
    let pair = app.keys.current();
    let key = credential(&pair, headers)?;
    if !grant::allows(&pair.1, &key, ids.ws(), need) {
        return Err(HubApiError::Forbidden(
            "the key has no grant for this workspace and plugin",
        ));
    }
    Ok(Credential {
        key,
        ws: ids.ws().to_owned(),
        plugin: ids.plugin().map(str::to_owned),
    })
}

//! The credential and the authorization decision, and nothing else.
//!
//! Two functions and one layer: [`credential`] turns the `Authorization` header into the key's
//! **name** (or a refusal), [`authorize`] adds the grant lookup and the path ids, and
//! [`authorize_middleware`] runs the decision for every `/v1` request before any handler does. The
//! layer is what makes "authorization before existence" a property of the router rather than of each
//! handler's discipline: a 403 is answered from the request line and the two credential files alone,
//! and no handler can have read the workspace first.
//!
//! **Order is the whole point** (§5.2): no key or an unknown key is 401; a key with no grant covering
//! the path's workspace and plugin is 403; only then may a missing workspace, plugin or record be
//! 404. A key therefore learns that a workspace exists only if it may read or write it, and the two
//! 403s are byte-identical whichever workspace was named — `no_grant_is_403_before_any_404` and
//! `refusal_bytes_are_identical_with_and_without_the_workspace` pin exactly that.

pub mod grant;
pub mod need;
pub mod path;

pub use grant::Need;
pub use path::Ids;

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::HeaderMap;
use axum::http::header;
use axum::middleware::Next;
use axum::response::Response;

use crate::app::App;
use crate::error::HubApiError;
use crate::keys::Pair;

/// Who a request is, once the credential and the grant both hold.
///
/// The layer puts this in the request's extensions, so a handler reads the identity it was already
/// given instead of parsing a header a second time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    /// The key's name, never its hash and never the key itself.
    pub key: String,
    /// The workspace the path names, or the empty string on a path that names none.
    pub ws: String,
    /// The plugin the path names, when it names one.
    pub plugin: Option<String>,
}

/// The name of the key the request carries, or a refusal.
///
/// Zero `Authorization` headers is 401 and more than one is **400**: RFC 9110 makes a message with
/// two of the same field invalid, and §5.2 requires exactly this, which is the one behaviour the hub
/// does that graph-server's own credential check does not
/// (`docs/decisions/graph-hub.md:120-121`).
///
/// Every other refusal of a credential is the same 401, so a caller cannot tell an unknown key from
/// a malformed one.
pub fn credential(pair: &Arc<Pair>, headers: &HeaderMap) -> Result<String, HubApiError> {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let Some(first) = values.next() else {
        return Err(HubApiError::Unauthorized("missing or unknown API key"));
    };
    if values.next().is_some() {
        return Err(HubApiError::BadRequest("one Authorization header at most"));
    }
    let text = first
        .to_str()
        .map_err(|_| HubApiError::Unauthorized("missing or unknown API key"))?;
    let token =
        App::credential_of(text).ok_or(HubApiError::Unauthorized("missing or unknown API key"))?;
    pair.0
        .name_of(token)
        .map(str::to_owned)
        .ok_or(HubApiError::Unauthorized("missing or unknown API key"))
}

/// The credential and the grant, or the first refusal in §5.2's order.
///
/// `need` is what the route requires and `ids` the path's checked ids. The grant lookup is on the
/// key's **name**: `KeySet` has no enumerable key list (`entries` is private,
/// `server/graph-server/src/keys.rs:60`), so a key with no grant is denied without the hub ever
/// learning that the key exists.
pub fn authorize(
    app: &Arc<App>,
    headers: &HeaderMap,
    ids: &Ids,
    need: &Need,
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

/// The layer every `/v1` request passes through: the decision, before any handler.
///
/// The keyring's `Arc` is cloned *inside* the request, never held by the layer, so a `SIGHUP` that
/// lands mid-request leaves that request on the pair it started with — the case
/// `a_relay_still_holds_a_key_set_across_a_sighup` pins.
///
/// A path §5.2 does not name is passed straight through to the router's 404: there is no grant for a
/// route that does not exist, so there is nothing to refuse first. A path §5.2 *does* name but whose
/// ids are malformed is a 400 or a 422 from [`path::ids_of`], which is §4's own rule and comes before
/// the grant lookup because a malformed id is the request's own fault.
pub async fn authorize_middleware(
    State(app): State<Arc<App>>,
    mut request: Request,
    next: Next,
) -> Result<Response, HubApiError> {
    // The path ids first, and only a 404 among their refusals is passed through: an unknown path is
    // the router's business, while a malformed id is §4's own 400 or 422 and must not be answered as
    // though the route did not exist.
    let ids = match path::ids_of(request.uri()) {
        Ok(ids) => ids,
        Err(HubApiError::NotFound(_)) => return Ok(next.run(request).await),
        Err(refused) => return Err(refused),
    };
    let need = need::need_for(request.method(), &ids);
    let credential = authorize(&app, request.headers(), &ids, &need)?;
    request.extensions_mut().insert(credential);
    crate::hooks::pause_after_admit(&app.hooks, "authorize").await;
    Ok(next.run(request).await)
}

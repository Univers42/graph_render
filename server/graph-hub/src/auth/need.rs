//! Which grant each row of §5.2's table needs, read off the method and the path alone.
//!
//! This is the whole of "authorization before existence": the need is decided from the request line,
//! before any handler runs and before any store call, so a workspace that is not there cannot change
//! the answer. A shape §5.2 does not name is `None`, and the caller lets it reach the 404 fallback:
//! there is no grant for a route that does not exist.

use axum::http::{Method, Uri};

use crate::auth::path::{Ids, ids_of};
use crate::grants::Need;

/// What one request line needs of its key, or `None` for a path §5.2 does not name.
pub fn need_of(method: &Method, uri: &Uri) -> Option<Need<'_>> {
    let ids = ids_of(uri).ok()?;
    Some(need_for(method, &ids))
}

/// The need of a known path, by the method §5.2's table gives it.
///
/// The `records` page is the one row whose grant reads "`write:<plugin>` or read", and that is what
/// [`Need::Write`] already means: a write grant covers the workspace's reads
/// ([`crate::auth::grant::covers`]), and a plain read grant is a [`Need::Read`]. So the two are one
/// lookup and not two.
fn need_for(method: &Method, ids: &Ids) -> Need<'_> {
    let ws_is_empty = ids.ws().is_empty();
    let plugin = ids.plugin_or_empty();
    match (method, ws_is_empty) {
        // `/v1/meta` and `/v1/workspaces` name no workspace, so §5.2's "any key" is `Need::Read` on
        // the empty workspace: a key with no grant at all is refused, and the route itself decides
        // what the key may see inside.
        (&Method::GET, true) => Need::Read,
        // `PUT /v1/workspaces/{ws}` is the only admin row: a workspace create.
        (&Method::PUT, _) if plugin.is_empty() => Need::Admin,
        // Every other read is a workspace read, and `plugin.is_empty()` covers `/graph`,
        // `/changes`, `/events`, `/layout` and `GET .../plugins`.
        (&Method::GET, false) if plugin.is_empty() => Need::Read,
        (&Method::GET, false) => Need::Write(plugin),
        // A manifest PUT and a batch POST both need `write:<plugin>`.
        (_, false) => Need::Write(plugin),
        // `POST /v1/workspaces/{ws}/layout` names a workspace and no plugin, and its grant is read.
        (_, true) => Need::Read,
    }
}

/// `true` when the path is one §5.2's table names, whatever the method.
///
/// The shape check is separate from the need because a *wrong method* on a real path is the router's
/// 405, and the router knows the path; the need is only asked for a request the table gives a grant.
pub fn is_known_path(uri: &Uri) -> bool {
    ids_of(uri).is_ok()
}
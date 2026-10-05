//! The path ids a request carries, checked once before anything else reads them.
//!
//! One extractor over the request's own `Uri`, because §5.2's grant is decided on the workspace
//! **and** the plugin, and both must be checked before the decision rather than after it. The
//! segment vocabulary is fixed by §5.2's route table, so the parser is a `match` over it and never
//! a guess about arity.
//!
//! Each segment is refused in one fixed order: empty, then a `\0`, then the contract's own
//! `check_*_id`.
//!
//! The `\0` is the hub's own line (Decision 8): §4 makes a percent-decoded NUL in a path id a 400,
//! and `check_record_id` (`crates/graph-contract/src/hub/ids.rs:52-63`) only tests for empty and
//! for `:`, so it **accepts** `\0`. `check_workspace_id` and friends refuse it only incidentally,
//! byte by byte, which is not a rule anyone can rely on. `crates/**` is out of this slice's paths,
//! so the refusal lives here.
//!
//! Caveat: an unrecognized path shape is a 404 rather than a 400, because the fallback answers it
//! before any id is read: there is no grant for a route that does not exist, so §5.2's order has
//! nothing to say about it.

use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::Uri;
use axum::http::request::Parts;
use percent_encoding::percent_decode_str;

use crate::app::App;
use crate::error::HubApiError;

/// Every id the path named, checked. An empty `Option` is a path that named none.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Ids {
    ws: String,
    plugin: Option<String>,
    collection: Option<String>,
    record: Option<String>,
}

impl Ids {
    /// The workspace, or the empty string on a path that names none (`/v1/meta`, `/v1/workspaces`).
    pub fn ws(&self) -> &str {
        &self.ws
    }

    /// The plugin, when the path named one.
    pub fn plugin(&self) -> Option<&str> {
        self.plugin.as_deref()
    }

    /// The collection, when the path named one.
    pub fn collection(&self) -> Option<&str> {
        self.collection.as_deref()
    }

    /// The record id, when the path named one.
    pub fn record(&self) -> Option<&str> {
        self.record.as_deref()
    }

    /// The plugin, or the empty string on a path that names none.
    ///
    /// A workspace-scoped read route grants on `read`, which needs no plugin name at all; the
    /// empty string is never a legal plugin id, so it can never be mistaken for one.
    pub fn plugin_or_empty(&self) -> &str {
        self.plugin.as_deref().unwrap_or("")
    }
}

/// The checked ids of a path, or the first refusal in it.
///
/// The shapes are §5.2's table verbatim, from `/v1` down. `{collection}` is checked with
/// `check_collection_id`, which is what refuses `B.coll` in a **path**; `B.coll` inside a **body**
/// is `Batch::check`'s job in the store, and the hub only relays the store's `HubError`.
pub fn ids_of(uri: &Uri) -> Result<Ids, HubApiError> {
    let segments = split(uri.path())?;
    let segments: Vec<&str> = segments.iter().map(String::as_str).collect();
    match segments.as_slice() {
        ["meta"] => Ok(Ids::default()),
        ["workspaces"] => Ok(Ids::default()),
        ["workspaces", ws] => Ok(Ids {
            ws: workspace(ws)?,
            ..Ids::default()
        }),
        ["workspaces", ws, "plugins"] => Ok(Ids {
            ws: workspace(ws)?,
            ..Ids::default()
        }),
        ["workspaces", ws, kind] if NO_PLUGIN_KINDS.contains(kind) => Ok(Ids {
            ws: workspace(ws)?,
            ..Ids::default()
        }),
        ["workspaces", ws, "plugins", plugin] => plugin_ids(ws, plugin),
        ["workspaces", ws, "plugins", plugin, kind] if *kind == "batches" || *kind == "records" => {
            plugin_ids(ws, plugin)
        }
        ["workspaces", ws, "records", plugin, collection, id] => {
            let mut ids = plugin_ids(ws, plugin)?;
            ids.collection = Some(collection_id(collection)?);
            ids.record = Some(record_id(id)?);
            Ok(ids)
        }
        _ => Err(HubApiError::NotFound("no such route")),
    }
}

/// The workspace-scoped routes of §5.2's table that name no plugin: `/graph`, `/changes`,
/// `/events` and `/layout`.
const NO_PLUGIN_KINDS: [&str; 4] = ["graph", "changes", "events", "layout"];

/// `ws` plus `plugin`, both checked.
fn plugin_ids(ws: &str, plugin: &str) -> Result<Ids, HubApiError> {
    Ok(Ids {
        ws: workspace(ws)?,
        plugin: Some(plugin_id(plugin)?),
        ..Ids::default()
    })
}

/// The segments after `/v1`, percent-decoded.
fn split(path: &str) -> Result<Vec<String>, HubApiError> {
    let rest = path
        .strip_prefix("/v1/")
        .or_else(|| path.strip_prefix("/v1"))
        .ok_or(HubApiError::NotFound("no such route"))?;
    rest.split('/')
        .map(percent_decode_str)
        .map(|decoded| decoded.decode_utf8().map(|text| text.into_owned()))
        .collect::<Result<Vec<String>, _>>()
        .map_err(|_| HubApiError::BadRequest("a path id is not UTF-8"))
}

/// A workspace id.
fn workspace(value: &str) -> Result<String, HubApiError> {
    check_segment(
        value,
        "workspace id",
        graph_contract::hub::check_workspace_id,
    )
}

/// A plugin id.
fn plugin_id(value: &str) -> Result<String, HubApiError> {
    check_segment(value, "plugin id", graph_contract::hub::check_plugin_id)
}

/// A collection id.
fn collection_id(value: &str) -> Result<String, HubApiError> {
    check_segment(
        value,
        "collection id",
        graph_contract::hub::check_collection_id,
    )
}

/// A record id.
fn record_id(value: &str) -> Result<String, HubApiError> {
    check_segment(value, "record id", graph_contract::hub::check_record_id)
}

/// One segment: not empty, no NUL, then the contract's own check.
///
/// The order is fixed: a NUL is a 400 from §4 whatever the grammar says, and a grammar fault is a
/// 422 because `HubError::status()` already decided that (`crates/graph-contract/src/hub/error.rs:66-73`).
fn check_segment(
    value: &str,
    what: &'static str,
    check: fn(&str) -> Result<(), graph_contract::hub::HubError>,
) -> Result<String, HubApiError> {
    if value.is_empty() {
        return Err(HubApiError::BadRequest("a path id is empty"));
    }
    if value.contains('\0') {
        return Err(HubApiError::BadRequest("a path id holds a NUL character"));
    }
    check(value).map_err(|error| HubApiError::Invalid {
        path: String::new(),
        what: format!("{what}: {error}"),
    })?;
    Ok(value.to_owned())
}

/// The extractor every route that needs ids takes.
#[derive(Debug, Clone)]
pub struct PathIds(pub Ids);

impl FromRequestParts<Arc<App>> for PathIds {
    type Rejection = HubApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &Arc<App>,
    ) -> Result<Self, Self::Rejection> {
        Ok(PathIds(ids_of(&parts.uri)?))
    }
}

//! `POST /v1/layout`'s query and `Accept` header, read strictly: an unknown parameter, a
//! repeated one or an id no registry holds is a 400 before any byte of the body is read.

use axum::http::HeaderValue;
use graph_core::registry::{self, Capability};
use percent_encoding::percent_decode_str;

use crate::error::ApiError;

/// The binary face's media type (`docs/contract/binary-layout.md`).
pub const SNAPSHOT_TYPE: &str = "application/vnd.graph-motor.snapshot";

/// Which ingest reader the body goes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The provisional node/edge JSON (`gm_build`'s reader).
    Studio,
    /// The phase-10 ingest contract (`gm_build_contract`'s reader).
    Contract,
}

/// Which face of the snapshot the response carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    /// `Snapshot::to_bytes`.
    Binary,
    /// `graph_contract::canonical_json::to_json`.
    Json,
}

impl Face {
    /// The response's `Content-Type`.
    pub fn content_type(self) -> &'static str {
        match self {
            Self::Binary => SNAPSHOT_TYPE,
            Self::Json => "application/json",
        }
    }
}

/// A checked layout request: every id resolved against its registry.
#[derive(Debug, Clone, Copy)]
pub struct LayoutQuery {
    /// The registered layout.
    pub layout: &'static Capability,
    /// Post indices into `graph_wasm::post::CAPABILITIES`, in the order given.
    pub posts: PostChain,
    /// The ingest reader.
    pub source: Source,
}

/// At most one of each registered post, so a request cannot ask for unbounded passes; held
/// inline because the registry is small and fixed.
pub type PostChain = ([u32; MAX_POSTS], usize);

/// The longest post chain a request may name: the post registry's own length.
pub const MAX_POSTS: usize = graph_wasm::post::CAPABILITIES.len();

/// Parses the query of `POST /v1/layout`.
pub fn parse(query: Option<&str>) -> Result<LayoutQuery, ApiError> {
    let (mut layout, mut post, mut source) = (None, None, None);
    for (key, value) in pairs(query.unwrap_or(""))? {
        let slot = match key.as_str() {
            "layout" => &mut layout,
            "post" => &mut post,
            "source" => &mut source,
            _ => return Err(ApiError::bad_request(format!("unknown parameter `{key}`"))),
        };
        if slot.replace(value).is_some() {
            return Err(ApiError::bad_request(format!("`{key}` given twice")));
        }
    }
    let layout = layout.ok_or_else(|| ApiError::bad_request("missing `layout`"))?;
    Ok(LayoutQuery {
        layout: find_layout(&layout)?,
        posts: post.as_deref().map_or(Ok(([0; MAX_POSTS], 0)), posts)?,
        source: parse_source(source.as_deref())?,
    })
}

/// Picks the face from `Accept`. No header, `*/*` and `application/*` mean the binary face,
/// the one the hash is taken over.
///
/// Caveat: the first media range that names a face wins and `q` weights are ignored, so
/// `application/json;q=0, */*` answers JSON. Direction: a client gets a face it listed, never
/// one it did not. Escape hatch: list the wanted face first.
pub fn face(accept: Option<&HeaderValue>) -> Result<Face, ApiError> {
    let Some(value) = accept else {
        return Ok(Face::Binary);
    };
    let text = value.to_str().map_err(|_| not_acceptable())?;
    for range in text.split(',') {
        let media = range.split(';').next().unwrap_or("").trim();
        if media.eq_ignore_ascii_case(SNAPSHOT_TYPE)
            || media.eq_ignore_ascii_case("application/*")
            || media == "*/*"
        {
            return Ok(Face::Binary);
        }
        if media.eq_ignore_ascii_case("application/json") {
            return Ok(Face::Json);
        }
    }
    Err(not_acceptable())
}

fn not_acceptable() -> ApiError {
    let message = format!("Accept names neither {SNAPSHOT_TYPE} nor application/json");
    ApiError::new(axum::http::StatusCode::NOT_ACCEPTABLE, "NotAcceptable", message)
}

/// The query's `key=value` pairs, percent-decoded. `+` is kept literally: no id contains a
/// space, so a `+` read as one would only turn one unknown id into another.
fn pairs(query: &str) -> Result<Vec<(String, String)>, ApiError> {
    let mut out = Vec::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        out.push((decode(key)?, decode(value)?));
    }
    Ok(out)
}

fn decode(text: &str) -> Result<String, ApiError> {
    let decoded = percent_decode_str(text).decode_utf8();
    decoded
        .map(|cow| cow.into_owned())
        .map_err(|_| ApiError::bad_request("the query is not UTF-8 once decoded"))
}

fn find_layout(id: &str) -> Result<&'static Capability, ApiError> {
    registry::find(id).ok_or_else(|| {
        let status = axum::http::StatusCode::BAD_REQUEST;
        ApiError::new(status, "UnknownLayoutId", format!("no layout `{id}`"))
    })
}

/// Each id of `post=a,b` as its registry index. Unknown is `IndexOutOfRange`, the code the
/// wasm motor answers an unregistered post with.
fn posts(list: &str) -> Result<PostChain, ApiError> {
    let mut chain = ([0u32; MAX_POSTS], 0usize);
    for id in list.split(',') {
        if chain.1 == MAX_POSTS {
            return Err(ApiError::bad_request(format!("at most {MAX_POSTS} posts")));
        }
        let index = (0..graph_wasm::post::count())
            .find(|&i| graph_wasm::post::id_at(i) == Some(id))
            .ok_or_else(|| {
                let status = axum::http::StatusCode::BAD_REQUEST;
                ApiError::new(status, "IndexOutOfRange", format!("no post `{id}`"))
            })?;
        chain.0[chain.1] = index;
        chain.1 += 1;
    }
    Ok(chain)
}

fn parse_source(source: Option<&str>) -> Result<Source, ApiError> {
    match source {
        None | Some("studio") => Ok(Source::Studio),
        Some("contract") => Ok(Source::Contract),
        Some(other) => Err(ApiError::bad_request(format!("unknown source `{other}`"))),
    }
}

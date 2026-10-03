//! The query string and the `Accept` header, read strictly (Verdict condition 6): an unknown or
//! repeated parameter, a second POST id or an id no registry holds is refused before any byte
//! of the body is read, and so is an API key in the query (condition 9).

use crate::error::ApiError;
use crate::motor::{self, Code, Source};
use axum::http::{HeaderMap, header};
use percent_encoding::percent_decode_str;

/// The binary face's media type (`docs/contract/binary-layout.md`).
pub const SNAPSHOT_TYPE: &str = "application/vnd.graph-motor.snapshot";
/// The canonical JSON face's media type.
pub const JSON_TYPE: &str = "application/json";

/// Parameter names a key is commonly sent under. Refused whatever their value.
const KEY_NAMES: [&str; 6] = [
    "key",
    "api_key",
    "apikey",
    "api-key",
    "access_token",
    "token",
];

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
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Binary => SNAPSHOT_TYPE,
            Self::Json => JSON_TYPE,
        }
    }
}

/// `POST /v1/layout`'s parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutQuery {
    /// `layout=`, a registered layout id.
    pub layout: String,
    /// `post=`, at most one registered POST id.
    pub post: Option<String>,
    /// `source=studio|contract`, `studio` by default.
    pub source: Source,
}

/// The decoded `(name, value)` pairs, a key among them refused. The message never quotes the
/// value: it may be a key.
pub fn pairs(raw: Option<&str>) -> Result<Vec<(String, String)>, ApiError> {
    let mut out = Vec::new();
    for pair in raw.unwrap_or("").split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').unwrap_or((pair, ""));
        let (name, value) = (decode(name)?, decode(value)?);
        let lower = name.to_ascii_lowercase();
        if KEY_NAMES.contains(&lower.as_str()) || value.starts_with(crate::keys::KEY_PREFIX) {
            return Err(ApiError::bad_request(
                "an API key in the query string is refused; send `Authorization: Bearer <key>`",
            ));
        }
        if out.iter().any(|(seen, _)| *seen == name) {
            return Err(ApiError::bad_request("a query parameter is repeated"));
        }
        out.push((name, value));
    }
    Ok(out)
}

/// Refuses any parameter: `/v1/meta` takes none.
pub fn none(pairs: &[(String, String)]) -> Result<(), ApiError> {
    match pairs.first() {
        None => Ok(()),
        Some(_) => Err(ApiError::bad_request("this route takes no query parameter")),
    }
}

/// `POST /v1/layout`'s parameters, each id checked against the motor's registries.
pub fn layout(pairs: &[(String, String)]) -> Result<LayoutQuery, ApiError> {
    let (mut layout, mut post, mut source) = (None, None, Source::Ingest);
    for (name, value) in pairs {
        match name.as_str() {
            "layout" => {
                layout = Some(registered(
                    value,
                    motor::layout_ids(),
                    Code::UnknownLayoutId,
                )?)
            }
            "post" => post = Some(post_id(value)?),
            "source" => source = source_of(value)?,
            _ => return Err(ApiError::bad_request("unknown query parameter")),
        }
    }
    let layout = layout.ok_or_else(|| ApiError::bad_request("`layout=<id>` is required"))?;
    Ok(LayoutQuery {
        layout,
        post,
        source,
    })
}

/// The face `Accept` asks for. No header, or `*/*`, is the binary face; a range with `q=0`
/// excludes its face; a header neither face satisfies is a 406. The most specific range that
/// matches a face sets its weight (RFC 9110 §12.5.1); on a tie the binary face wins.
pub fn face(headers: &HeaderMap) -> Result<Face, ApiError> {
    let mut ranges = Vec::new();
    for value in headers.get_all(header::ACCEPT) {
        let text = value.to_str().map_err(|_| ApiError::not_acceptable())?;
        for range in text.split(',').map(str::trim).filter(|r| !r.is_empty()) {
            ranges.push(media_range(range)?);
        }
    }
    if ranges.is_empty() {
        return Ok(Face::Binary);
    }
    let (binary, json) = (weight(&ranges, SNAPSHOT_TYPE), weight(&ranges, JSON_TYPE));
    match (binary, json) {
        (b, j) if b > 0 && b >= j => Ok(Face::Binary),
        (_, j) if j > 0 => Ok(Face::Json),
        _ => Err(ApiError::not_acceptable()),
    }
}

/// `type/subtype` lowercased and its weight in thousandths.
fn media_range(range: &str) -> Result<(String, u16), ApiError> {
    let mut parts = range.split(';').map(str::trim);
    let media = parts.next().unwrap_or("").to_ascii_lowercase();
    let mut quality = 1000;
    for param in parts {
        let (name, value) = param.split_once('=').ok_or_else(ApiError::not_acceptable)?;
        if name.trim().eq_ignore_ascii_case("q") {
            quality = thousandths(value.trim()).ok_or_else(ApiError::not_acceptable)?;
        }
    }
    Ok((media, quality))
}

/// An RFC 9110 qvalue (`0`, `1`, `0.5`, `0.125`) in thousandths.
fn thousandths(text: &str) -> Option<u16> {
    let (whole, fraction) = text.split_once('.').unwrap_or((text, ""));
    let digits = fraction.len() <= 3 && fraction.bytes().all(|b| b.is_ascii_digit());
    let value = match whole {
        "0" if digits => format!("{fraction:0<3}").parse().ok()?,
        "1" if fraction.bytes().all(|b| b == b'0') && fraction.len() <= 3 => 1000,
        _ => return None,
    };
    Some(value)
}

/// The weight `ranges` give `media`: from the exact range, else `application/*`, else `*/*`.
fn weight(ranges: &[(String, u16)], media: &str) -> u16 {
    let lookup = |wanted: &str| {
        ranges
            .iter()
            .find(|(range, _)| range == wanted)
            .map(|r| r.1)
    };
    lookup(media)
        .or_else(|| lookup("application/*"))
        .or_else(|| lookup("*/*"))
        .unwrap_or(0)
}

fn registered<'a>(
    id: &str,
    mut known: impl Iterator<Item = &'a str>,
    code: Code,
) -> Result<String, ApiError> {
    if known.any(|registered| registered == id) {
        Ok(id.to_owned())
    } else {
        Err(motor::refusal(code))
    }
}

/// One POST id: the motor runs at most one pass per request (condition 6).
fn post_id(value: &str) -> Result<String, ApiError> {
    if value.contains(',') {
        return Err(ApiError::bad_request("`post=` takes one POST id"));
    }
    registered(value, motor::post_ids(), Code::IndexOutOfRange)
}

fn source_of(value: &str) -> Result<Source, ApiError> {
    match value {
        "studio" => Ok(Source::Ingest),
        "contract" => Ok(Source::Contract),
        _ => Err(ApiError::bad_request("`source=` is `studio` or `contract`")),
    }
}

/// Percent-decodes one component. `+` stays a `+`: no id contains a space.
fn decode(text: &str) -> Result<String, ApiError> {
    percent_decode_str(text)
        .decode_utf8()
        .map(|decoded| decoded.into_owned())
        .map_err(|_| ApiError::bad_request("the query string is not UTF-8"))
}

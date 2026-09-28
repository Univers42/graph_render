//! The semantic face of a snapshot: canonical JSON, the contract any third-party frontend
//! reads. `docs/contract/binary-layout.md` §"The JSON face" states the rules and
//! `docs/contract/snapshot-schema.json` the shape; this module is both directions.
//!
//! Canonical means one text per snapshot: compact, keys sorted by bytes at every depth,
//! arrays in snapshot order, one trailing newline. A float is written with Rust's `f32`
//! `Display` — the shortest decimal that reads back as the same `f32`, never an exponent
//! — and read back rounded straight to `f32`. An edge's endpoints are written as node
//! **ids**, never positions: the dense index does not leave the binary face.
//!
//! [`to_json`] then [`from_json`] gives back the same snapshot, so binary → JSON →
//! binary is byte-exact; `graph-cli roundtrip` proves it over the seed sweep.

use crate::binary::Snapshot;
use crate::geometry::{EdgeGeometry, EdgeGeometryKind, NodeGeometry, NodeGeometryKind, Paths};
use crate::snapshot::SnapshotError;
use crate::version::{FormatVersion, NewerMajor, UNVERSIONED, check_readable};
use core::fmt::{self, Write};

mod parse;
mod read;
#[cfg(feature = "codegen")]
pub mod schema;

pub use parse::{Value, parse};

/// Why a JSON document was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    /// Not JSON.
    Syntax {
        /// Byte offset of the fault.
        at: u32,
        /// What was wrong there.
        what: &'static str,
    },
    /// JSON, but not the snapshot's shape.
    Shape {
        /// Dotted path of the offending value, e.g. `geometry.nodes.x[3]`.
        path: String,
        /// What was wrong with it.
        what: &'static str,
    },
    /// Written in a newer major than this reader knows.
    Version(NewerMajor),
    /// The shape was right; the snapshot it describes breaks a rule of the layout.
    Snapshot(SnapshotError),
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax { at, what } => write!(f, "not JSON at byte {at}: {what}"),
            Self::Shape { path, what } if path.is_empty() => write!(f, "the document: {what}"),
            Self::Shape { path, what } => write!(f, "{path}: {what}"),
            Self::Version(newer) => newer.fmt(f),
            Self::Snapshot(err) => err.fmt(f),
        }
    }
}

/// Every node kind with the name the JSON face spells it with.
pub const NODE_KINDS: [(NodeGeometryKind, &str); 3] = [
    (NodeGeometryKind::Point, "Point"),
    (NodeGeometryKind::Circle, "Circle"),
    (NodeGeometryKind::Box, "Box"),
];

/// Every edge kind with the name the JSON face spells it with.
pub const EDGE_KINDS: [(EdgeGeometryKind, &str); 3] = [
    (EdgeGeometryKind::Line, "Line"),
    (EdgeGeometryKind::Polyline, "Polyline"),
    (EdgeGeometryKind::Curve, "Curve"),
];

fn node_kind_name(kind: NodeGeometryKind) -> &'static str {
    NODE_KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or("", |(_, name)| name)
}

fn edge_kind_name(kind: EdgeGeometryKind) -> &'static str {
    EDGE_KINDS
        .iter()
        .find(|(k, _)| *k == kind)
        .map_or("", |(_, name)| name)
}

/// The canonical JSON text of `snapshot`.
pub fn to_json(snapshot: &Snapshot) -> String {
    let p = snapshot.parts();
    let endpoints = |ends: &[u32]| strings(ends.iter().map(|&i| p.node_ids.get(i).unwrap_or("")));
    let edges = object(vec![
        ("id", strings(p.edge_ids.iter())),
        ("source", endpoints(&p.source)),
        ("target", endpoints(&p.target)),
    ]);
    let geometry = object(vec![
        ("edges", edge_geometry(&p.edges)),
        ("nodes", node_geometry(&p.nodes)),
    ]);
    let nodes = object(vec![("id", strings(p.node_ids.iter()))]);
    let version = object(vec![
        ("major", p.version.major.to_string()),
        ("minor", p.version.minor.to_string()),
    ]);
    let mut out = object(vec![
        ("edges", edges),
        ("geometry", geometry),
        ("nodes", nodes),
        ("version", version),
    ]);
    out.push('\n');
    out
}

/// Reads a JSON document — canonical or not, keys in any order — into a snapshot.
/// The version is read first: a newer major is refused before its shape is looked at.
pub fn from_json(text: &str) -> Result<Snapshot, JsonError> {
    read::snapshot(parse::parse(text)?)
}

/// An object of already-written members, in byte order of their keys whatever order
/// they are listed in.
fn object(mut members: Vec<(&str, String)>) -> String {
    members.sort_by(|a, b| a.0.cmp(b.0));
    let mut out = String::from("{");
    for (i, (key, value)) in members.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        string(&mut out, key);
        out.push(':');
        out.push_str(value);
    }
    out.push('}');
    out
}

fn node_geometry(nodes: &NodeGeometry) -> String {
    let mut members = vec![("kind", quoted(node_kind_name(nodes.kind())))];
    members.extend(nodes.columns().into_iter().map(|(name, c)| (name, f32s(c))));
    object(members)
}

fn edge_geometry(edges: &EdgeGeometry) -> String {
    let mut members = vec![("kind", quoted(edge_kind_name(edges.kind())))];
    let paths = |p: &Paths| [("offsets", u32s(&p.offsets)), ("pts", f32s(&p.pts))];
    match edges {
        EdgeGeometry::Line => {}
        EdgeGeometry::Polyline(p) => members.extend(paths(p)),
        EdgeGeometry::Curve { degree, paths: p } => {
            members.push(("degree", degree.to_string()));
            members.extend(paths(p));
        }
    }
    object(members)
}

fn strings<'a>(items: impl Iterator<Item = &'a str>) -> String {
    let mut out = String::from("[");
    for (i, item) in items.enumerate() {
        if i > 0 {
            out.push(',');
        }
        string(&mut out, item);
    }
    out.push(']');
    out
}

fn f32s(values: &[f32]) -> String {
    list(values)
}

fn u32s(values: &[u32]) -> String {
    list(values)
}

/// `[v0,v1,…]`, each value by its `Display`: shortest round-trip for `f32`, plain
/// decimal for `u32`.
fn list<T: fmt::Display>(values: &[T]) -> String {
    let mut out = String::from("[");
    for (i, value) in values.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{value}");
    }
    out.push(']');
    out
}

fn quoted(text: &str) -> String {
    let mut out = String::new();
    string(&mut out, text);
    out
}

/// A JSON string: `"` and `\` escaped, the five controls JSON names by letter as
/// letters, every other control as `\u00xx`, everything else as raw UTF-8.
fn string(out: &mut String, text: &str) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// The version a document declares, or [`UNVERSIONED`] when it declares none; refused
/// when newer than this reader.
fn declared(version: Option<FormatVersion>) -> Result<FormatVersion, JsonError> {
    let version = version.unwrap_or(UNVERSIONED);
    check_readable(version).map_err(JsonError::Version)?;
    Ok(version)
}

#[cfg(test)]
mod tests;

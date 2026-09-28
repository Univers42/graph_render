//! From a parsed JSON value to a [`Snapshot`]: the shape of
//! `docs/contract/snapshot-schema.json`, every member required except `version`, and
//! `notes` below format 0.3 (absent there means no notes); no member the shape does not
//! name.

use super::parse::Value;
use super::{EDGE_KINDS, JsonError, NODE_KINDS, declared};
use crate::binary::{Snapshot, SnapshotParts, StringTable};
use crate::geometry::{EdgeGeometry, EdgeGeometryKind, NodeGeometry, NodeGeometryKind, Paths};
use crate::notes::{Notes, carries_notes};
use crate::version::FormatVersion;
use std::collections::BTreeMap;

pub(super) fn snapshot(root: Value) -> Result<Snapshot, JsonError> {
    let mut root = Object::of(root, String::new())?;
    let version = version(root.maybe("version"))?;
    let mut nodes = root.object("nodes")?;
    let node_ids = table(nodes.take("id")?, "nodes.id")?;
    nodes.finish()?;
    let mut edges = root.object("edges")?;
    let edge_ids = table(edges.take("id")?, "edges.id")?;
    let mut index = BTreeMap::new();
    for (id, i) in node_ids.iter().zip(0..) {
        index.entry(id).or_insert(i);
    }
    let source = endpoints(edges.take("source")?, "edges.source", &index)?;
    let target = endpoints(edges.take("target")?, "edges.target", &index)?;
    edges.finish()?;
    let mut geometry = root.object("geometry")?;
    let node_geometry = node_geometry(geometry.object("nodes")?)?;
    let edge_geometry = edge_geometry(geometry.object("edges")?)?;
    geometry.finish()?;
    let notes = notes(root.maybe("notes"), version)?;
    root.finish()?;
    Snapshot::new(SnapshotParts {
        version,
        node_ids,
        edge_ids,
        source,
        target,
        nodes: node_geometry,
        edges: edge_geometry,
        notes,
    })
    .map_err(JsonError::Snapshot)
}

fn shape(path: &str, what: &'static str) -> JsonError {
    JsonError::Shape {
        path: path.to_owned(),
        what,
    }
}

/// An object's members still to be read, and where it sits in the document.
struct Object {
    path: String,
    members: Vec<(String, Value)>,
}

impl Object {
    fn of(value: Value, path: String) -> Result<Self, JsonError> {
        match value {
            Value::Object(members) => Ok(Self { path, members }),
            _ => Err(shape(&path, "must be an object")),
        }
    }

    fn at(&self, key: &str) -> String {
        if self.path.is_empty() {
            key.to_owned()
        } else {
            format!("{}.{key}", self.path)
        }
    }

    fn maybe(&mut self, key: &str) -> Option<Value> {
        let i = self.members.iter().position(|(k, _)| k == key)?;
        Some(self.members.remove(i).1)
    }

    fn take(&mut self, key: &str) -> Result<(Value, String), JsonError> {
        let path = self.at(key);
        match self.maybe(key) {
            Some(value) => Ok((value, path)),
            None => Err(shape(&path, "is missing")),
        }
    }

    fn object(&mut self, key: &str) -> Result<Self, JsonError> {
        let (value, path) = self.take(key)?;
        Self::of(value, path)
    }

    /// `Ok` when every member has been read: a member this shape does not name is
    /// refused, not skipped.
    fn finish(self) -> Result<(), JsonError> {
        match self.members.first() {
            Some((key, _)) => Err(shape(&self.at(key), "is not part of the snapshot shape")),
            None => Ok(()),
        }
    }
}

fn items((value, path): (Value, String)) -> Result<(Vec<Value>, String), JsonError> {
    match value {
        Value::Array(items) => Ok((items, path)),
        _ => Err(shape(&path, "must be an array")),
    }
}

fn text(value: Value, path: &str) -> Result<String, JsonError> {
    match value {
        Value::String(text) => Ok(text),
        _ => Err(shape(path, "must be a string")),
    }
}

fn table(member: (Value, String), column: &'static str) -> Result<StringTable, JsonError> {
    let (items, path) = items(member)?;
    let ids: Vec<String> = items
        .into_iter()
        .enumerate()
        .map(|(i, v)| text(v, &format!("{path}[{i}]")))
        .collect::<Result<_, _>>()?;
    StringTable::from_strs(column, ids.iter().map(String::as_str)).map_err(JsonError::Snapshot)
}

/// Endpoint ids, each turned into its node's position; an id that is no node's is
/// refused here, since it has no position to become.
fn endpoints(
    member: (Value, String),
    path: &str,
    index: &BTreeMap<&str, u32>,
) -> Result<Vec<u32>, JsonError> {
    let (items, _) = items(member)?;
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.into_iter().enumerate() {
        let at = format!("{path}[{i}]");
        let id = text(item, &at)?;
        out.push(
            *index
                .get(id.as_str())
                .ok_or_else(|| shape(&at, "is not a node id"))?,
        );
    }
    Ok(out)
}

fn number(value: Value, path: &str) -> Result<String, JsonError> {
    match value {
        Value::Number(text) => Ok(text),
        _ => Err(shape(path, "must be a number")),
    }
}

/// A `u32` written as a plain integer: no sign, fraction or exponent.
fn u32_of(value: Value, path: &str) -> Result<u32, JsonError> {
    let text = number(value, path)?;
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(shape(path, "must be an integer from 0 to 4294967295"));
    }
    text.parse()
        .map_err(|_| shape(path, "must be an integer from 0 to 4294967295"))
}

/// A float rounded straight from its decimal to the nearest `f32`. A decimal past the
/// `f32` range reads as ±∞, which [`Snapshot::new`] then refuses (D9).
fn f32_of(value: Value, path: &str) -> Result<f32, JsonError> {
    let text = number(value, path)?;
    text.parse().map_err(|_| shape(path, "must be a number"))
}

fn list<T>(
    member: (Value, String),
    read: fn(Value, &str) -> Result<T, JsonError>,
) -> Result<Vec<T>, JsonError> {
    let (items, path) = items(member)?;
    items
        .into_iter()
        .enumerate()
        .map(|(i, v)| read(v, &format!("{path}[{i}]")))
        .collect()
}

/// The declared version, refused when newer before anything else in the document is
/// looked at; [`super::UNVERSIONED`] when there is none.
fn version(value: Option<Value>) -> Result<FormatVersion, JsonError> {
    let Some(value) = value else {
        return declared(None);
    };
    let mut object = Object::of(value, "version".into())?;
    let (major, major_path) = object.take("major")?;
    let (minor, minor_path) = object.take("minor")?;
    let found = declared(Some(FormatVersion {
        major: u32_of(major, &major_path)?,
        minor: u32_of(minor, &minor_path)?,
    }))?;
    object.finish()?;
    Ok(found)
}

/// The notes columns: required from 0.3, absent below it read as none. Whether they are
/// a valid, canonical set is [`Snapshot::new`]'s to say, as for the binary face.
fn notes(value: Option<Value>, version: FormatVersion) -> Result<Notes, JsonError> {
    let Some(value) = value else {
        if carries_notes(version) {
            return Err(shape("notes", "is missing"));
        }
        return Ok(Notes::default());
    };
    let mut object = Object::of(value, "notes".into())?;
    let code = list(object.take("code")?, u32_of)?;
    let index = list(object.take("index")?, u32_of)?;
    object.finish()?;
    Ok(Notes { code, index })
}

fn kind<K: Copy>(object: &mut Object, kinds: &[(K, &str)]) -> Result<K, JsonError> {
    let (value, path) = object.take("kind")?;
    let name = text(value, &path)?;
    kinds
        .iter()
        .find(|(_, n)| *n == name)
        .map(|(k, _)| *k)
        .ok_or_else(|| shape(&path, "is not a kind this version knows"))
}

fn node_geometry(mut object: Object) -> Result<NodeGeometry, JsonError> {
    let mut column = |key| list(object.take(key)?, f32_of);
    let (x, y) = (column("x")?, column("y")?);
    let geometry = match kind(&mut object, &NODE_KINDS)? {
        NodeGeometryKind::Point => NodeGeometry::Point { x, y },
        NodeGeometryKind::Circle => NodeGeometry::Circle {
            x,
            y,
            r: list(object.take("r")?, f32_of)?,
        },
        NodeGeometryKind::Box => NodeGeometry::Box {
            x,
            y,
            w: list(object.take("w")?, f32_of)?,
            h: list(object.take("h")?, f32_of)?,
        },
    };
    object.finish()?;
    Ok(geometry)
}

fn edge_geometry(mut object: Object) -> Result<EdgeGeometry, JsonError> {
    let geometry = match kind(&mut object, &EDGE_KINDS)? {
        EdgeGeometryKind::Line => EdgeGeometry::Line,
        EdgeGeometryKind::Polyline => EdgeGeometry::Polyline(paths(&mut object)?),
        EdgeGeometryKind::Curve => {
            let (degree, path) = object.take("degree")?;
            let degree = u32_of(degree, &path)?;
            EdgeGeometry::Curve {
                degree,
                paths: paths(&mut object)?,
            }
        }
    };
    object.finish()?;
    Ok(geometry)
}

fn paths(object: &mut Object) -> Result<Paths, JsonError> {
    Ok(Paths {
        offsets: list(object.take("offsets")?, u32_of)?,
        pts: list(object.take("pts")?, f32_of)?,
    })
}

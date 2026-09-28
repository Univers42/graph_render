//! The records a host hands in (`GraphNode` / `GraphEdge`, `src/core/types.ts:25-60`),
//! and borrowed views of one node or edge — from a record or from a topology's columns.

use crate::columns::NodeKind;
use crate::edgekind::EdgeKind;

/// One raw node, as a host hands it in. `fields` (lazy, never compared) is not carried.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeRecord {
    /// Stable id; the only identity that crosses the wire.
    pub id: String,
    /// What the node is.
    pub kind: NodeKind,
    /// Source database; `None` for free notes and synthetic hubs.
    pub database_id: Option<String>,
    /// Backend that owns the truth.
    pub source: String,
    /// Human label.
    pub label: String,
    /// Secondary label (status, category).
    pub group: Option<String>,
    /// Visual weight, 0..1 by convention.
    pub weight: f64,
    /// Optimistic-concurrency version; a JS `number`.
    pub version: f64,
    /// A note overlay exists.
    pub has_note: bool,
    /// Raw icon value.
    pub icon: Option<String>,
}

/// One raw edge. `source` and `target` are node ids, and may dangle.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeRecord {
    /// Content-addressed id.
    pub id: String,
    /// Source node id.
    pub source: String,
    /// Target node id.
    pub target: String,
    /// Relationship flavour.
    pub kind: EdgeKind,
    /// Relation name or tag value.
    pub label: String,
    /// Layout pull and drawn thickness.
    pub strength: f64,
    /// Whether orientation matters.
    pub directed: bool,
    /// Backing edge-row id, if any.
    pub record_id: Option<String>,
    /// The wire type named the child first (`child_of`): `source` is the child and
    /// `target` the parent. Set it with [`crate::child_first_from_type`]; only the
    /// hierarchy reads it, and `source`/`target` stay as they arrived.
    pub child_first: bool,
}

/// A node's fields, borrowed — from a [`NodeRecord`] or from a topology's columns.
/// What `nodesEqual` and the fixture writer read.
#[derive(Debug, Clone, Copy)]
pub struct NodeView<'a> {
    /// Stable id.
    pub id: &'a str,
    /// Kind.
    pub kind: NodeKind,
    /// Source database.
    pub database_id: Option<&'a str>,
    /// Owning backend.
    pub source: &'a str,
    /// Label.
    pub label: &'a str,
    /// Secondary label.
    pub group: Option<&'a str>,
    /// Visual weight.
    pub weight: f64,
    /// Version.
    pub version: f64,
    /// Note overlay flag.
    pub has_note: bool,
    /// Icon.
    pub icon: Option<&'a str>,
}

/// An edge's fields, borrowed, with endpoints as node ids.
#[derive(Debug, Clone, Copy)]
pub struct EdgeView<'a> {
    /// Content-addressed id.
    pub id: &'a str,
    /// Source node id.
    pub source: &'a str,
    /// Target node id.
    pub target: &'a str,
    /// Kind.
    pub kind: EdgeKind,
    /// Label.
    pub label: &'a str,
    /// Strength.
    pub strength: f64,
    /// Directed flag.
    pub directed: bool,
    /// Backing row id.
    pub record_id: Option<&'a str>,
    /// `source` is the child (`child_of`).
    pub child_first: bool,
}

impl NodeRecord {
    /// This record's fields, borrowed.
    pub fn view(&self) -> NodeView<'_> {
        NodeView {
            id: &self.id,
            kind: self.kind,
            database_id: self.database_id.as_deref(),
            source: &self.source,
            label: &self.label,
            group: self.group.as_deref(),
            weight: self.weight,
            version: self.version,
            has_note: self.has_note,
            icon: self.icon.as_deref(),
        }
    }
}

impl EdgeRecord {
    /// This record's fields, borrowed.
    pub fn view(&self) -> EdgeView<'_> {
        EdgeView {
            id: &self.id,
            source: &self.source,
            target: &self.target,
            kind: self.kind,
            label: &self.label,
            strength: self.strength,
            directed: self.directed,
            record_id: self.record_id.as_deref(),
            child_first: self.child_first,
        }
    }
}

/// Terse record builders for the unit tests of every module.
#[cfg(test)]
pub(crate) mod build {
    use super::*;

    /// A `record` node in database `db` (none when empty), source `pg`, weight 0.5.
    pub fn node(id: &str, db: &str) -> NodeRecord {
        NodeRecord {
            id: id.into(),
            kind: NodeKind::Record,
            database_id: (!db.is_empty()).then(|| db.into()),
            source: "pg".into(),
            label: format!("L{id}"),
            group: None,
            weight: 0.5,
            version: 0.0,
            has_note: false,
            icon: None,
        }
    }

    /// `node` with another kind.
    pub fn kind(id: &str, kind: NodeKind) -> NodeRecord {
        NodeRecord {
            kind,
            ..node(id, "")
        }
    }

    /// An undirected `relation` edge.
    pub fn edge(id: &str, source: &str, target: &str) -> EdgeRecord {
        EdgeRecord {
            id: id.into(),
            source: source.into(),
            target: target.into(),
            kind: EdgeKind::Relation,
            label: String::new(),
            strength: 0.5,
            directed: false,
            record_id: None,
            child_first: false,
        }
    }
}

//! Reading one row at a time, borrowed.
//!
//! Every accessor here reads a cell the value pass has already refused unless it is legal:
//! a string index that names an entry, a boolean that is `0` or `1`, a float that is finite,
//! an endpoint row below the node count. That is the whole reason [`decode`] checks the
//! whole document once and these do not: they are then plain arithmetic over slices that
//! cannot be out of range, and a row number past the counts is the one thing they refuse.
//!
//! A row comes two ways. [`NodeCells`] and [`EdgeCells`] keep every string as its table
//! entry, so a host that meets the same entry on a million rows can resolve it once.
//! [`NodeRow`] and [`EdgeRow`] are the same cells with every entry read as text.
//!
//! [`decode`]: super::decode

use super::check::{cell, float};
use super::layout::ABSENT;
use super::{ColumnsDoc, EdgeRow, NodeRow};

/// One node row with every string left as its string-table entry. Each entry is below
/// `string_count`: [`decode`](super::decode) refused the document otherwise.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeCells {
    /// Entry of the stable id.
    pub id: u32,
    /// Entry of the kind name.
    pub kind: u32,
    /// Entry of the source database, `None` where the document said `u32::MAX`.
    pub database_id: Option<u32>,
    /// Entry of the owning backend.
    pub source: u32,
    /// Entry of the label.
    pub label: u32,
    /// Entry of the secondary label, `None` where the document said `u32::MAX`.
    pub group: Option<u32>,
    /// Visual weight.
    pub weight: f64,
    /// Version.
    pub version: f64,
    /// Note overlay flag.
    pub has_note: bool,
    /// Entry of the icon, `None` where the document said `u32::MAX`.
    pub icon: Option<u32>,
}

/// One edge row with every string left as its string-table entry, endpoints as node rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeCells {
    /// Entry of the content-addressed id.
    pub id: u32,
    /// Dense index of the source node.
    pub source_row: u32,
    /// Dense index of the target node.
    pub target_row: u32,
    /// Entry of the kind name.
    pub kind: u32,
    /// Entry of the label.
    pub label: u32,
    /// Strength.
    pub strength: f64,
    /// Directed flag.
    pub directed: bool,
    /// Entry of the backing row id, `None` where the document said `u32::MAX`.
    pub record_id: Option<u32>,
    /// `source_row` is the child (`child_of`).
    pub child_first: bool,
}

/// An optional column's cell: `u32::MAX` is absent, anything else is an entry.
fn entry(column: &[u8], row: u32) -> Option<u32> {
    Some(cell(column, row)).filter(|&index| index != ABSENT)
}

impl<'a> ColumnsDoc<'a> {
    /// Node row `row` as table entries, or `None` if `row` is past `node_count`.
    pub fn node_cells(&self, row: u32) -> Option<NodeCells> {
        if row >= self.nodes {
            return None;
        }
        let c = &self.columns;
        Some(NodeCells {
            id: cell(c.id, row),
            kind: cell(c.kind, row),
            database_id: entry(c.database, row),
            source: cell(c.source, row),
            label: cell(c.label, row),
            group: entry(c.group, row),
            weight: float(c.weight, row),
            version: float(c.version, row),
            has_note: cell(c.has_note, row) == 1,
            icon: entry(c.icon, row),
        })
    }

    /// Edge row `row` as table entries, or `None` if `row` is past `edge_count`.
    pub fn edge_cells(&self, row: u32) -> Option<EdgeCells> {
        if row >= self.edges {
            return None;
        }
        let c = &self.columns;
        Some(EdgeCells {
            id: cell(c.edge_id, row),
            source_row: cell(c.edge_source, row),
            target_row: cell(c.edge_target, row),
            kind: cell(c.edge_kind, row),
            label: cell(c.edge_label, row),
            strength: float(c.strength, row),
            directed: cell(c.directed, row) == 1,
            record_id: entry(c.record_id, row),
            child_first: cell(c.child_first, row) == 1,
        })
    }

    /// Node row `row`, or `None` if `row` is past `node_count`.
    ///
    /// `None` is unreachable for a caller that walks `0..node_count()`, which is the only
    /// way a host gets rows: it is here so a host that keeps a row number in hand from
    /// somewhere else gets a refusal rather than a trap.
    pub fn node(&self, row: u32) -> Option<NodeRow<'a>> {
        let c = self.node_cells(row)?;
        Some(NodeRow {
            id: self.text(c.id)?,
            kind: self.text(c.kind)?,
            database_id: self.optional(c.database_id),
            source: self.text(c.source)?,
            label: self.text(c.label)?,
            group: self.optional(c.group),
            weight: c.weight,
            version: c.version,
            has_note: c.has_note,
            icon: self.optional(c.icon),
        })
    }

    /// Edge row `row`, or `None` if `row` is past `edge_count`.
    pub fn edge(&self, row: u32) -> Option<EdgeRow<'a>> {
        let c = self.edge_cells(row)?;
        Some(EdgeRow {
            id: self.text(c.id)?,
            source_row: c.source_row,
            target_row: c.target_row,
            kind: self.text(c.kind)?,
            label: self.text(c.label)?,
            strength: c.strength,
            directed: c.directed,
            record_id: self.optional(c.record_id),
            child_first: c.child_first,
        })
    }

    /// An optional entry as text.
    fn optional(&self, entry: Option<u32>) -> Option<&'a str> {
        entry.and_then(|index| self.text(index))
    }
}

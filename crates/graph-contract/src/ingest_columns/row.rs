//! Reading one row at a time, borrowed.
//!
//! Every accessor here reads a cell the value pass has already refused unless it is legal:
//! a string index that names an entry, a boolean that is `0` or `1`, a float that is finite,
//! an endpoint row below the node count. That is the whole reason [`decode`] checks the
//! whole document once and these do not: they are then plain arithmetic over slices that
//! cannot be out of range, and a row number past the counts is the one thing they refuse.

use super::check::{cell, float};
use super::{ColumnsDoc, EdgeRow, NodeRow};

impl<'a> ColumnsDoc<'a> {
    /// Node row `row`, or `None` if `row` is past `node_count`.
    ///
    /// `None` is unreachable for a caller that walks `0..node_count()`, which is the only
    /// way a host gets rows: it is here so a host that keeps a row number in hand from
    /// somewhere else gets a refusal rather than a trap.
    pub fn node(&self, row: u32) -> Option<NodeRow<'a>> {
        if row >= self.nodes {
            return None;
        }
        let c = &self.columns;
        Some(NodeRow {
            id: self.text(cell(c.id, row))?,
            kind: self.text(cell(c.kind, row))?,
            database_id: self.optional(c.database, row),
            source: self.text(cell(c.source, row))?,
            label: self.text(cell(c.label, row))?,
            group: self.optional(c.group, row),
            weight: float(c.weight, row),
            version: float(c.version, row),
            has_note: cell(c.has_note, row) == 1,
            icon: self.optional(c.icon, row),
        })
    }

    /// Edge row `row`, or `None` if `row` is past `edge_count`.
    pub fn edge(&self, row: u32) -> Option<EdgeRow<'a>> {
        if row >= self.edges {
            return None;
        }
        let c = &self.columns;
        Some(EdgeRow {
            id: self.text(cell(c.edge_id, row))?,
            source_row: cell(c.edge_source, row),
            target_row: cell(c.edge_target, row),
            kind: self.text(cell(c.edge_kind, row))?,
            label: self.text(cell(c.edge_label, row))?,
            strength: float(c.strength, row),
            directed: cell(c.directed, row) == 1,
            record_id: self.optional(c.record_id, row),
            child_first: cell(c.child_first, row) == 1,
        })
    }
}

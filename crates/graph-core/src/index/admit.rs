//! The one admit path: where a row becomes a column entry, for both ingest paths.
//!
//! A row is admitted in two steps. `claim_*` files the interned id under the next dense row,
//! or reports it taken; `push_*` appends every column. Between them the caller interns the
//! row's other strings: [`index_model`](super::index_model) from the row's own `&str`s
//! (`admit_*` below), [`index_columns`](super::columns::index_columns) through a memo of the
//! document's string table. The arena numbers its slots in first-intern order, so the two
//! paths agree on every handle only if they intern in the same order — and that order is the
//! field order of [`InternedNode`] and [`InternedEdge`], which Rust evaluates as written.

use super::Topology;
use crate::arena::{CapacityError, Interned, StringArena};
use crate::columns::NodeKind;
use crate::edgekind::EdgeKind;
use crate::records::{EdgeFields, NodeView};

/// A node row with its strings interned. Built after `claim_node`, fields in this order.
#[derive(Debug, Clone, Copy)]
pub(super) struct InternedNode {
    pub(super) database: Option<Interned>,
    pub(super) source: Interned,
    pub(super) label: Interned,
    pub(super) group: Option<Interned>,
    pub(super) icon: Option<Interned>,
    pub(super) kind: NodeKind,
    pub(super) weight: f64,
    pub(super) version: f64,
    pub(super) has_note: bool,
}

/// An edge row with its strings interned. Built after `claim_edge`, fields in this order.
#[derive(Debug, Clone, Copy)]
pub(super) struct InternedEdge {
    pub(super) label: Interned,
    pub(super) record_id: Option<Interned>,
    pub(super) kind: EdgeKind,
    pub(super) strength: f64,
    pub(super) directed: bool,
    pub(super) child_first: bool,
}

impl Topology {
    /// Files `id` under the next node row, or `false` if a node already holds it. A `true`
    /// must be followed by `push_node`: the row is filed but no column has it yet.
    pub(super) fn claim_node(&mut self, id: Interned) -> Result<bool, CapacityError> {
        if self.node_ids.row(id).is_some() {
            return Ok(false);
        }
        let row = next_index(self.nodes.id.len(), "node index")?;
        self.node_ids.file(id, row);
        Ok(true)
    }

    /// Appends the node claimed as `id`.
    pub(super) fn push_node(&mut self, id: Interned, node: InternedNode) {
        let n = &mut self.nodes;
        n.id.push(id);
        n.database.push(node.database);
        n.source.push(node.source);
        n.label.push(node.label);
        n.group_label.push(node.group);
        n.icon.push(node.icon);
        n.kind.push(node.kind);
        n.weight.push(node.weight);
        n.version.push(node.version);
        n.has_note.push(node.has_note);
    }

    /// Files `id` under the next edge row, or `false` if an edge already holds it.
    pub(super) fn claim_edge(&mut self, id: Interned) -> Result<bool, CapacityError> {
        if self.edge_ids.row(id).is_some() {
            return Ok(false);
        }
        let row = next_index(self.edges.id.len(), "edge index")?;
        self.edge_ids.file(id, row);
        Ok(true)
    }

    /// Appends the edge claimed as `id`, joining the dense indices `at`.
    pub(super) fn push_edge(&mut self, id: Interned, at: (u32, u32), edge: InternedEdge) {
        let e = &mut self.edges;
        e.id.push(id);
        e.source.push(at.0);
        e.target.push(at.1);
        e.label.push(edge.label);
        e.record_id.push(edge.record_id);
        e.kind.push(edge.kind);
        e.strength.push(edge.strength);
        e.directed.push(edge.directed);
        e.child_first.push(edge.child_first);
    }

    /// Keeps `node` unless its id is taken: first wins (`model.ts:37-40`). One arena probe
    /// and one row lookup per node: a taken id is already interned, so `intern` adds nothing.
    ///
    /// `false` means the id was already there and nothing was appended — the drop
    /// `index_model` wants and `index_columns` refuses.
    pub(super) fn admit_node(&mut self, node: &NodeView<'_>) -> Result<bool, CapacityError> {
        let id = self.strings.intern(node.id)?;
        if !self.claim_node(id)? {
            return Ok(false);
        }
        let s = &mut self.strings;
        let interned = InternedNode {
            database: intern_opt(s, node.database_id)?,
            source: s.intern(node.source)?,
            label: s.intern(node.label)?,
            group: intern_opt(s, node.group)?,
            icon: intern_opt(s, node.icon)?,
            kind: node.kind,
            weight: node.weight,
            version: node.version,
            has_note: node.has_note,
        };
        self.push_node(id, interned);
        Ok(true)
    }

    /// Keeps `edge` at the already-resolved endpoint indices `at`, unless its id is taken
    /// (`model.ts:47-53`). `false` means the id was already there and nothing was appended.
    ///
    /// A dropped edge interns nothing, so it neither claims its id nor costs arena bytes: a
    /// taken id is already interned, and the caller resolves the endpoints first.
    pub(super) fn admit_edge(
        &mut self,
        at: (u32, u32),
        edge: &EdgeFields<'_>,
    ) -> Result<bool, CapacityError> {
        let id = self.strings.intern(edge.id)?;
        if !self.claim_edge(id)? {
            return Ok(false);
        }
        let s = &mut self.strings;
        let interned = InternedEdge {
            label: s.intern(edge.label)?,
            record_id: intern_opt(s, edge.record_id)?,
            kind: edge.kind,
            strength: edge.strength,
            directed: edge.directed,
            child_first: edge.child_first,
        };
        self.push_edge(id, at, interned);
        Ok(true)
    }
}

/// A dense index for the next entry of a set already holding `len`, or the refusal.
pub(super) fn next_index(len: usize, what: &'static str) -> Result<u32, CapacityError> {
    u32::try_from(len)
        .ok()
        .filter(|&i| i < u32::MAX)
        .ok_or(CapacityError { what })
}

fn intern_opt(
    strings: &mut StringArena,
    value: Option<&str>,
) -> Result<Option<Interned>, CapacityError> {
    value.map(|v| strings.intern(v)).transpose()
}

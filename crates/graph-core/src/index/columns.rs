//! [`index_columns`]: the indexed model from rows, for the columnar ingest document
//! (`docs/contract/ingest-columns.md`).
//!
//! Same admit path as [`index_model`](super::index_model) — `admit_node` and `admit_edge`
//! are the only places a row becomes a column entry — but the *policy* on a taken id is the
//! opposite. `index_model` drops the duplicate, first wins, because a JSON document may name
//! the same id twice and there is no answer a caller could act on. A columnar document cannot
//! afford that: its edge endpoints are **row numbers**, so a dropped row would renumber every
//! row after it and silently repoint every edge. A duplicate is therefore a refusal, which is
//! also what keeps the invariant this module exists for —
//!
//! > row `r` of the node columns is the node whose dense index is `r`.
//!
//! # What graph-core owns and graph-contract does not
//!
//! graph-core sees no wire format. The iterators below carry borrowed `&str` and resolved
//! kinds, so the decoder that produced them stays in `graph-contract` and this module stays
//! testable with plain arrays.

use super::Topology;
use crate::arena::CapacityError;
use crate::records::{NodeView, RowEdge};
use core::fmt;

/// Why a columnar document could not be indexed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColumnsRefusal {
    /// Row `row`'s node id is already taken. Never merged, never dropped: dropping it would
    /// renumber the rows after it and repoint every endpoint row that follows.
    DuplicateNodeId {
        /// The row that repeats an earlier id.
        row: u32,
    },
    /// Row `row`'s edge id is already taken.
    DuplicateEdgeId {
        /// The row that repeats an earlier edge id.
        row: u32,
    },
    /// An endpoint names a row at or past the last node admitted, which the document's own
    /// `node_count` check should have caught. Kept as a separate refusal so a caller that
    /// hands rows in the wrong order is told so instead of being told "duplicate id".
    EndpointRow {
        /// The edge row carrying it.
        row: u32,
    },
    /// The `u32` index space ran out, as in [`index_model`](super::index_model).
    Capacity(CapacityError),
}

impl From<CapacityError> for ColumnsRefusal {
    fn from(error: CapacityError) -> Self {
        Self::Capacity(error)
    }
}

impl fmt::Display for ColumnsRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateNodeId { row } => write!(f, "node row {row} repeats an id"),
            Self::DuplicateEdgeId { row } => write!(f, "edge row {row} repeats an id"),
            Self::EndpointRow { row } => write!(f, "edge row {row} names a node row that is gone"),
            Self::Capacity(inner) => fmt::Display::fmt(inner, f),
        }
    }
}

/// Indexes nodes and edges given as rows, in document order.
///
/// `nodes` and `edges` must both be in the document's own row order: row `r` admitted is
/// dense index `r`. Refuses a repeated node or edge id rather than dropping it, which is what
/// makes an edge's endpoint row mean what it says.
///
/// **Caveat:** the reservation counts rows only. The string arena is left to grow, where
/// [`index_model`](super::index_model) also over-reserves it by exactly the distinct
/// non-id strings the document holds — a graph whose labels are all different grows it once
/// either way.
pub fn index_columns<'a>(
    mut nodes: impl ExactSizeIterator<Item = NodeView<'a>>,
    mut edges: impl ExactSizeIterator<Item = RowEdge<'a>>,
) -> Result<Topology, ColumnsRefusal> {
    let (node_rows, edge_rows) = (nodes.len(), edges.len());
    let mut topology = Topology::with_row_capacity(node_rows, edge_rows);
    for node in &mut nodes {
        let row = topology.node_count();
        if !topology.admit_node(&node)? {
            return Err(ColumnsRefusal::DuplicateNodeId { row });
        }
    }
    for edge in &mut edges {
        let row = topology.edge_count();
        push_edge(&mut topology, row, &edge)?;
    }
    topology.finish()?;
    Ok(topology)
}

/// The one shared edge push: the endpoint-row check, then `admit_edge`, then the refusal a
/// taken id earns. `row` is the edge's dense index, which is also its document row.
fn push_edge(topology: &mut Topology, row: u32, edge: &RowEdge<'_>) -> Result<(), ColumnsRefusal> {
    let nodes = topology.node_count();
    if edge.source_row >= nodes || edge.target_row >= nodes {
        return Err(ColumnsRefusal::EndpointRow { row });
    }
    let at = (edge.source_row, edge.target_row);
    if !topology.admit_edge(at, &edge.fields())? {
        return Err(ColumnsRefusal::DuplicateEdgeId { row });
    }
    Ok(())
}

#[cfg(test)]
mod tests;

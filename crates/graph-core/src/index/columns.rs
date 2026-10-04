//! [`index_columns`]: the indexed model from rows, for the columnar ingest document
//! (`docs/contract/ingest-columns.md`).
//!
//! Same admit path as [`index_model`](super::index_model) — `claim_*` and `push_*`
//! (`index/admit.rs`) are the only places a row becomes a column entry — but the *policy* on
//! a taken id is the
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
//! graph-core sees no wire format. A row arrives as [`NodeCells`] or [`EdgeCells`], every
//! string an entry of an [`EntryTable`], so the decoder that produced them stays in
//! `graph-contract` and this module stays testable with a plain `[&str]`.

use super::Topology;
use super::admit::{InternedEdge, InternedNode};
use super::extend::Load;
use crate::arena::{CapacityError, StringArena};
use core::fmt;

pub use cells::{EdgeCells, EntryTable, NodeCells};
// `index_columns`'s own pieces, widened one level for `index::extend::columns`: the append
// path reaches them rather than reimplementing them, which is what keeps its intern order
// equal to `index_columns`'s (`docs/decisions/extend-columns.md`, condition 7).
pub(in crate::index) use cells::Entries;

/// One `GMX1` batch edge row: every string a table entry, both endpoints **entries naming node
/// ids**. This is where a batch differs from a whole document, whose endpoints are dense rows: a
/// batch's edge may name a node the graph already holds, and a row number would have leaked an
/// index that never crosses the wire.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BatchEdgeCells {
    /// Entry of the content-addressed id.
    pub id: u32,
    /// Entry of the source node's id.
    pub source_entry: u32,
    /// Entry of the target node's id.
    pub target_entry: u32,
    /// Entry of the kind name.
    pub kind: u32,
    /// Entry of the label.
    pub label: u32,
    /// Entry of the backing row id, if any.
    pub record_id: Option<u32>,
    /// Strength.
    pub strength: f64,
    /// Directed flag.
    pub directed: bool,
    /// `source_entry` is the child (`child_of`).
    pub child_first: bool,
}

/// What `nodes` and `edges` would add to a graph, counting every string as new: the same count
/// [`extend::Load::of_batch`](super::extend::Load) makes over records, over entries instead of
/// `String`s. Six strings per node (id, database, source, label, group, icon), three per edge
/// (id, label, record id). A kind name is **not** one: it resolves to a `NodeKind` or an
/// `EdgeKind` and is never interned, on either path. Nor is an endpoint's entry — it names a
/// node id, counted where that node is carried, or already interned by the graph.
///
/// `pub(in crate::index)` so the columns twin of [`Topology::extend`](super::extend)'s capacity
/// test can pin this rule on the production count.
pub(in crate::index) fn batch_load<T, N, E>(table: &T, nodes: N, edges: E) -> Load
where
    T: EntryTable + ?Sized,
    N: ExactSizeIterator<Item = NodeCells>,
    E: ExactSizeIterator<Item = BatchEdgeCells>,
{
    let mut load = Load {
        strings: 0,
        bytes: 0,
        nodes: nodes.len() as u64,
        edges: edges.len() as u64,
    };
    for node in nodes {
        let named = [node.id, node.source, node.label];
        for entry in [node.database_id, node.group, node.icon] {
            counted(&mut load, table, entry);
        }
        for entry in named {
            counted(&mut load, table, Some(entry));
        }
    }
    for edge in edges {
        for entry in [Some(edge.id), Some(edge.label), edge.record_id] {
            counted(&mut load, table, entry);
        }
    }
    load
}

/// One named string counted as new. An entry the table does not have counts for nothing: the
/// decoder refused such a batch before graph-core saw it, and a count is not the place to
/// refuse it twice.
fn counted<T: EntryTable + ?Sized>(load: &mut Load, table: &T, entry: Option<u32>) {
    if let Some(text) = entry.and_then(|entry| table.text(entry)) {
        load.strings += 1;
        load.bytes += text.len() as u64;
    }
}

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
    /// A cell names a string-table entry past the table's end.
    TableEntry {
        /// The entry named.
        entry: u32,
    },
    /// Row `row`'s kind is no node kind's name.
    NodeKind {
        /// The node row carrying it.
        row: u32,
    },
    /// Row `row`'s kind is no edge kind's name.
    EdgeKind {
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
            Self::TableEntry { entry } => write!(f, "string-table entry {entry} does not exist"),
            Self::NodeKind { row } => write!(f, "node row {row} names no node kind"),
            Self::EdgeKind { row } => write!(f, "edge row {row} names no edge kind"),
            Self::Capacity(inner) => fmt::Display::fmt(inner, f),
        }
    }
}

/// Indexes nodes and edges given as rows over `table`, in document order.
///
/// `nodes` and `edges` must both be in the document's own row order: row `r` admitted is
/// dense index `r`. Refuses a repeated node or edge id rather than dropping it, which is what
/// makes an edge's endpoint row mean what it says.
///
/// The arena reserves once, up front, instead of rehashing as it grows; at 1M nodes the
/// growth was 17% of the build (`docs/measurements/perf-open-intern.md`).
pub fn index_columns<T: EntryTable + ?Sized>(
    table: &T,
    nodes: impl ExactSizeIterator<Item = NodeCells>,
    edges: impl ExactSizeIterator<Item = EdgeCells>,
) -> Result<Topology, ColumnsRefusal> {
    let mut topology = Topology::with_row_capacity(nodes.len(), edges.len());
    let reserved = reserved_entries(table.entries(), nodes.len(), edges.len());
    topology.strings = StringArena::with_capacity(reserved, table.bytes());
    let mut entries = Entries::new(table, reserved);
    for node in nodes {
        index_node(&mut topology, &mut entries, &node)?;
    }
    for edge in edges {
        index_edge(&mut topology, &mut entries, &edge)?;
    }
    topology.finish()?;
    Ok(topology)
}

/// How many distinct strings to reserve for: never more than the rows can name — six per node
/// (id, database, source, label, group, icon) and three per edge (id, label, record id) — so a
/// table far larger than its rows (the contract does not forbid one) reserves for the rows.
///
/// **Caveat:** over-counts by repeated entries and absent optional cells, and is never more
/// than an all-distinct build would grow to.
pub(in crate::index) fn reserved_entries(entries: usize, nodes: usize, edges: usize) -> usize {
    let named = nodes
        .saturating_mul(6)
        .saturating_add(edges.saturating_mul(3));
    entries.min(named)
}

/// Admits the next node row, interning its strings in `admit_node`'s order.
pub(in crate::index) fn index_node<T: EntryTable + ?Sized>(
    topology: &mut Topology,
    entries: &mut Entries<'_, T>,
    cells: &NodeCells,
) -> Result<(), ColumnsRefusal> {
    let row = topology.node_count();
    let id = entries.string(&mut topology.strings, cells.id)?;
    if !topology.claim_node(id)? {
        return Err(ColumnsRefusal::DuplicateNodeId { row });
    }
    let s = &mut topology.strings;
    let node = InternedNode {
        database: entries.optional(s, cells.database_id)?,
        source: entries.string(s, cells.source)?,
        label: entries.string(s, cells.label)?,
        group: entries.optional(s, cells.group)?,
        icon: entries.optional(s, cells.icon)?,
        kind: entries.node_kind(cells.kind, row)?,
        weight: cells.weight,
        version: cells.version,
        has_note: cells.has_note,
    };
    topology.push_node(id, node);
    Ok(())
}

/// Admits the next edge row: the endpoint-row check, then `admit_edge`'s intern order.
pub(in crate::index) fn index_edge<T: EntryTable + ?Sized>(
    topology: &mut Topology,
    entries: &mut Entries<'_, T>,
    cells: &EdgeCells,
) -> Result<(), ColumnsRefusal> {
    let (row, nodes) = (topology.edge_count(), topology.node_count());
    if cells.source_row >= nodes || cells.target_row >= nodes {
        return Err(ColumnsRefusal::EndpointRow { row });
    }
    let id = entries.string(&mut topology.strings, cells.id)?;
    if !topology.claim_edge(id)? {
        return Err(ColumnsRefusal::DuplicateEdgeId { row });
    }
    let s = &mut topology.strings;
    let edge = InternedEdge {
        label: entries.string(s, cells.label)?,
        record_id: entries.optional(s, cells.record_id)?,
        kind: entries.edge_kind(cells.kind, row)?,
        strength: cells.strength,
        directed: cells.directed,
        child_first: cells.child_first,
    };
    topology.push_edge(id, (cells.source_row, cells.target_row), edge);
    Ok(())
}

mod cells;

#[cfg(test)]
mod tests;

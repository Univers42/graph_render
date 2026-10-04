//! [`Topology::extend_columns`]: one `GMX1` batch appended to a live graph
//! (`docs/decisions/extend-columns.md`) — the append [`Topology::extend`](super::extend)
//! makes, from rows whose strings are entries of one table instead of owned `String`s. Same
//! validation, same refusals, same intern order, and afterwards the topology is byte-identical
//! to the record path over the same rows; `extend_columns_matches_extend` is that claim.
//!
//! [`index_node`], [`index_edge`] and [`Entries`] are *reached*, not copied — `index_columns`’s
//! own pieces, widened to `pub(in crate::index)` — and they are what holds this path’s intern
//! order equal to `admit_node`’s. On top of them this adds what `index_columns` leaves to
//! `finish()`: the three `csr.push_row()` calls, `degree.push(0)` and `group_node` per node,
//! `file_edge` per edge.
//!
//! [`index_columns`] itself is **not** called: it builds a *fresh* `Topology` and admits as it
//! walks, so on a live handle it would replace the graph. Everything here is validated before
//! the first intern — an id probe, an endpoint resolution and a kind lookup per row, all reads.
//!
//! [`index_columns`]: super::columns::index_columns

use super::{ExtendError, Load, Topology, row_of};
use crate::arena::{CapacityError, FixedState};
use crate::index::columns::{
    ColumnsRefusal, EdgeCells, Entries, EntryTable, NodeCells, index_edge, index_node,
    reserved_entries,
};
use core::fmt;
use indexmap::IndexSet;

/// One `GMX1` edge row: every string a table entry, both endpoints **entries naming node ids**.
/// This is where a batch differs from a whole document, whose endpoints are dense rows: a
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

/// Why a `GMX1` batch was refused. The topology is unchanged on every variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchRefusal {
    /// One of the four refusals [`Topology::extend`](super::extend) gives, verbatim.
    Extend(ExtendError),
    /// Batch node `index`'s kind names no node kind.
    NodeKind {
        /// The node's position in the batch.
        index: u32,
    },
    /// Batch edge `index`'s kind names no edge kind.
    EdgeKind {
        /// The edge's position in the batch.
        index: u32,
    },
    /// A cell names a string-table entry the table does not have.
    TableEntry {
        /// The entry named.
        entry: u32,
    },
}

impl fmt::Display for BatchRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Extend(inner) => fmt::Display::fmt(inner, f),
            Self::NodeKind { index } => write!(f, "batch node {index}: kind names no node kind"),
            Self::EdgeKind { index } => write!(f, "batch edge {index}: kind names no edge kind"),
            Self::TableEntry { entry } => write!(f, "string-table entry {entry} does not exist"),
        }
    }
}

/// A `CapacityError` is the one refusal both paths spell the same way, so the append's three
/// call sites need one conversion rather than three matches.
fn capacity(error: CapacityError) -> BatchRefusal {
    BatchRefusal::Extend(error.into())
}

/// What `nodes` and `edges` would add to a graph, counting every string as new: the same count
/// [`Load::of_batch`] makes over records, over entries instead of `String`s. Six strings per node
/// (id, database, source, label, group, icon), three per edge (id, label, record id). A kind
/// name is **not** one: it resolves to a `NodeKind` or an `EdgeKind` and is never interned, on
/// either path. Nor is an endpoint's entry — it names a node id, counted where that node is
/// carried, or already interned by the graph.
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
        let cells = [node.id, node.database_id, node.source, node.label, node.group, node.icon];
        for entry in cells.map(Some) {
            counted(&mut load, table, entry);
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

/// Everything the validate pass resolved, kept for the append: the batch's node ids (what an
/// endpoint naming one resolves against), every edge's dense endpoints, and the per-entry memo
/// with every kind name already resolved against it.
struct Plan<'t, T: EntryTable + ?Sized> {
    table: &'t T,
    entries: Entries<'t, T>,
    ids: IndexSet<&'t str, FixedState>,
    at: Vec<(u32, u32)>,
    /// The dense row the batch's first node will hold, and its first edge's.
    base: (u32, u32),
}

impl<'t, T: EntryTable + ?Sized> Plan<'t, T> {
    fn new(table: &'t T, topology: &Topology, nodes: usize, edges: usize) -> Self {
        let reserved = reserved_entries(table.entries(), nodes, edges);
        Self {
            table,
            entries: Entries::new(table, reserved),
            ids: IndexSet::with_capacity_and_hasher(nodes, FixedState::default()),
            at: Vec::with_capacity(edges),
            base: (topology.node_count(), topology.edge_count()),
        }
    }

    /// The batch's node ids, refused on the first one the graph or the batch already has, and
    /// every node row's kind name resolved — all of it reading, none of it writing.
    fn check_nodes<N: Iterator<Item = NodeCells>>(
        &mut self,
        topology: &Topology,
        nodes: N,
    ) -> Result<(), BatchRefusal> {
        for (index, node) in (0..).zip(nodes) {
            let named = [node.id, node.source, node.label];
            let optional = [node.database_id, node.group, node.icon];
            self.in_table([named[0], node.kind], optional)?;
            let id = self.text(node.id)?;
            if topology.node_index(id).is_some() || !self.ids.insert(id) {
                return Err(BatchRefusal::Extend(ExtendError::NodeId { index }));
            }
            // The kind name, resolved now against the same memo the append reads back. Every
            // entry is inside the table, so the only refusal left is the vocabulary's.
            self.entries
                .node_kind(node.kind, index)
                .map_err(|_| BatchRefusal::NodeKind { index })?;
        }
        Ok(())
    }

    /// The batch's edge ids, its kind names, and both endpoints as the dense rows they will
    /// hold. The rows are resolved here because this pass looks every endpoint up anyway to
    /// decide whether it names a node at all.
    fn check_edges<E: Iterator<Item = BatchEdgeCells>>(
        &mut self,
        topology: &Topology,
        edges: E,
    ) -> Result<(), BatchRefusal> {
        let mut ids = IndexSet::with_capacity_and_hasher(self.at.capacity(), FixedState::default());
        for (index, edge) in (0..).zip(edges) {
            self.in_table([edge.id, edge.kind], [edge.record_id, edge.source_entry, edge.target_entry])?;
            let id = self.text(edge.id)?;
            if topology.edge_index(id).is_some() || !ids.insert(id) {
                return Err(BatchRefusal::Extend(ExtendError::EdgeId { index }));
            }
            self.entries
                .edge_kind(edge.kind, index)
                .map_err(|_| BatchRefusal::EdgeKind { index })?;
            let ends = [edge.source_entry, edge.target_entry].map(|entry| {
                self.text(entry)
                    .map(|id| row_of(topology, &self.ids, self.base.0, id))
            });
            match ends {
                [Ok(Some(source)), Ok(Some(target))] => self.at.push((source, target)),
                [Err(error), _] | [_, Err(error)] => return Err(error),
                _ => return Err(BatchRefusal::Extend(ExtendError::Endpoint { index })),
            }
        }
        Ok(())
    }

    /// Every cell of one row inside the table. Resolving an entry interns and interning
    /// writes, so this rule is settled before the first intern rather than during it — which
    /// is also why [`text`](Self::text) below cannot come back empty.
    fn in_table(
        &self,
        required: [u32; 2],
        optional: [Option<u32>; 3],
    ) -> Result<(), BatchRefusal> {
        for cell in required.into_iter().map(Some).chain(optional) {
            if *cell as usize >= self.table.entries() {
                return Err(BatchRefusal::TableEntry { entry: *cell });
            }
        }
        Ok(())
    }

    /// The text `entry` names, which the two checks above have proved it names.
    fn text(&self, entry: u32) -> Result<&'t str, BatchRefusal> {
        self.table.text(entry).ok_or(BatchRefusal::TableEntry { entry })
    }

    /// A refusal from [`index_node`] or [`index_edge`] in this path's own rows: the validate
    /// pass settled every rule but capacity, so these row numbers become batch positions.
    fn refusal(&self, error: ColumnsRefusal) -> BatchRefusal {
        match error {
            ColumnsRefusal::Capacity(inner) => BatchRefusal::Extend(inner.into()),
            ColumnsRefusal::DuplicateNodeId { row } | ColumnsRefusal::NodeKind { row } => {
                BatchRefusal::Extend(ExtendError::NodeId { index: row - self.base.0 })
            }
            ColumnsRefusal::DuplicateEdgeId { row } | ColumnsRefusal::EdgeKind { row } => {
                BatchRefusal::Extend(ExtendError::EdgeId { index: row - self.base.1 })
            }
            ColumnsRefusal::EndpointRow { row } => {
                BatchRefusal::Extend(ExtendError::Endpoint { index: row - self.base.1 })
            }
            ColumnsRefusal::TableEntry { entry } => BatchRefusal::TableEntry { entry },
        }
    }
}

impl Topology {
    /// Appends one `GMX1` batch: nodes first, then edges in order, exactly as
    /// [`extend`](Topology::extend) appends records. On `Err` `self` is unchanged; an empty
    /// batch is `Ok` and changes nothing. The same four graph refusals as [`BatchRefusal::Extend`]
    /// and the same capacity check, so a refused batch has claimed no id and no arena slot.
    pub fn extend_columns<T, N, E>(
        &mut self,
        table: &T,
        nodes: N,
        edges: E,
    ) -> Result<(), BatchRefusal>
    where
        T: EntryTable + ?Sized,
        N: Clone + ExactSizeIterator<Item = NodeCells>,
        E: Clone + ExactSizeIterator<Item = BatchEdgeCells>,
    {
        let load = batch_load(table, nodes.clone(), edges.clone());
        Load::of(self).check(load).map_err(BatchRefusal::Extend)?;
        let mut plan = Plan::new(table, self, nodes.len(), edges.len());
        plan.check_nodes(self, nodes.clone())?;
        plan.check_edges(self, edges.clone())?;
        self.append_column_nodes(&mut plan, nodes)?;
        let at = std::mem::take(&mut plan.at);
        self.append_column_edges(&mut plan, edges, &at)
    }

    /// Each node row through `index_columns`'s own admit, then the empty CSR rows, the zero
    /// degree and the group `extend` gives it. Nothing here can refuse but the arena, whose
    /// limit [`batch_load`] has already cleared.
    fn append_column_nodes<T, N>(&mut self, plan: &mut Plan<'_, T>, nodes: N) -> Result<(), BatchRefusal>
    where
        T: EntryTable + ?Sized,
        N: Iterator<Item = NodeCells>,
    {
        for node in nodes {
            index_node(self, &mut plan.entries, &node).map_err(|e| plan.refusal(e))?;
            for csr in [&mut self.out, &mut self.inbound, &mut self.hierarchy] {
                csr.push_row().map_err(capacity)?;
            }
            self.nodes.degree.push(0);
            self.group_node(self.node_count() - 1);
        }
        Ok(())
    }

    /// Each edge row at the endpoints the validate pass resolved, through the same admit, then
    /// filed in the CSRs as `extend` files it. The one conversion the two row shapes need:
    /// entries naming node ids become the dense rows this file wrote them as.
    fn append_column_edges<T, E>(
        &mut self,
        plan: &mut Plan<'_, T>,
        edges: E,
        at: &[(u32, u32)],
    ) -> Result<(), BatchRefusal>
    where
        T: EntryTable + ?Sized,
        E: Iterator<Item = BatchEdgeCells>,
    {
        for (edge, (source, target)) in edges.zip(at.iter().copied()) {
            let cells = EdgeCells {
                id: edge.id,
                source_row: source,
                target_row: target,
                kind: edge.kind,
                label: edge.label,
                strength: edge.strength,
                directed: edge.directed,
                record_id: edge.record_id,
                child_first: edge.child_first,
            };
            index_edge(self, &mut plan.entries, &cells).map_err(|e| plan.refusal(e))?;
            self.file_edge(self.edge_count() - 1).map_err(capacity)?;
        }
        Ok(())
    }
}
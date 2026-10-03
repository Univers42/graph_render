//! The indexed model (`src/core/model/model.ts`): dense indices over stable ids, the
//! string arena, the SoA columns and three CSR adjacencies, built in one O(n + m) pass.
//!
//! A node's dense index is its admission order, the position the oracle's `Map` gives it,
//! filed under its interned id's arena slot ([`RowBySlot`]; never a `HashMap`, H2, D4). The
//! dense index is internal: only the stable string id crosses the wire.

use crate::arena::{CapacityError, FixedState, Interned, StringArena};
use crate::columns::{EdgeColumns, NodeColumns, NodeKind};
use crate::csr::Csr;
use crate::edgekind::EdgeKind;
use crate::records::{EdgeRecord, NodeRecord, NodeView};
#[cfg(test)]
use admit::next_index;
use indexmap::{IndexMap, IndexSet};
use slots::RowBySlot;

/// `GraphStats` (`types.ts:75-80`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Stats {
    /// Nodes kept after de-duplication.
    pub nodes: u32,
    /// Edges kept after de-duplication and dropping dangling ones.
    pub edges: u32,
    /// Distinct database ids among the kept nodes.
    pub databases: u32,
    /// Kept nodes of kind `note`.
    pub notes: u32,
}

/// The indexed graph. Build it with [`index_model`].
#[derive(Debug, Clone, Default)]
pub struct Topology {
    strings: StringArena,
    node_ids: RowBySlot,
    edge_ids: RowBySlot,
    nodes: NodeColumns,
    edges: EdgeColumns,
    out: Csr,
    inbound: Csr,
    hierarchy: Csr,
    by_database: IndexMap<Interned, Vec<u32>, FixedState>,
    notes: u32,
}

/// `indexModel` (`model.ts:36-71`): nodes de-duplicated first-wins, then edges kept in
/// order unless their id was already taken or an endpoint is missing.
///
/// **Caveat:** the reservation counts id fields only, the near-unique set. The arena
/// under-reserves by every distinct non-id string — a graph whose labels are all
/// different still grows it once — and `node_ids`/`edge_ids` over-reserve by exactly the
/// ids this pass drops: duplicate node ids, and edges whose endpoints are missing.
pub fn index_model(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Result<Topology, CapacityError> {
    let mut topology = Topology::with_row_capacity(nodes.len(), edges.len());
    let (strings, bytes) = size_hint(nodes, edges);
    topology.strings = StringArena::with_capacity(strings, bytes);
    for node in nodes {
        topology.admit_node(&node.view())?;
    }
    for edge in edges {
        // The endpoints are resolved before anything is interned, exactly as inside
        // `admit_edge`: a dangling edge must claim no id and cost no arena bytes.
        if let Some(at) = topology.endpoints(&edge.source, &edge.target) {
            topology.admit_edge(at, &edge.view().fields())?;
        }
    }
    topology.finish()?;
    Ok(topology)
}

/// The arena reservation for `nodes` and `edges`: how many distinct strings, and
/// how many bytes, the ids alone would hold — one pass, no hashing.
fn size_hint(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> (usize, usize) {
    let mut strings = 0usize;
    let mut bytes = 0usize;
    for node in nodes {
        strings += 1;
        bytes += node.id.len();
    }
    for edge in edges {
        strings += 1;
        bytes += edge.id.len();
    }
    (strings, bytes)
}

/// `emptyModel` (`model.ts:74-76`).
pub fn empty_model() -> Topology {
    Topology::default()
}

/// `nodesEqual` (`model.ts:10-32`): every field but `id`, floats compared with IEEE `==`
/// exactly as JS `===` compares them (`NaN` unequal to itself, `-0` equal to `0`).
pub fn nodes_equal(a: &NodeView<'_>, b: &NodeView<'_>) -> bool {
    a.kind == b.kind
        && a.database_id == b.database_id
        && a.source == b.source
        && a.label == b.label
        && a.group == b.group
        && a.weight == b.weight
        && a.version == b.version
        && a.has_note == b.has_note
        && a.icon == b.icon
}

impl Topology {
    /// An empty topology with room for `nodes` node rows and `edges` edge rows: the id sets
    /// and the four SoA columns. The arena is left to grow — a caller that knows the
    /// document's string shape replaces it, as [`index_model`] does.
    pub(super) fn with_row_capacity(nodes: usize, edges: usize) -> Self {
        Self {
            node_ids: RowBySlot::with_capacity(nodes),
            edge_ids: RowBySlot::with_capacity(edges),
            nodes: NodeColumns::with_capacity(nodes),
            edges: EdgeColumns::with_capacity(edges),
            ..Self::default()
        }
    }

    /// The tail every ingest path shares, in this order: the three adjacencies and the
    /// degree column, then the group column, `by_database` and the note count.
    pub(super) fn finish(&mut self) -> Result<(), CapacityError> {
        self.build_adjacency()?;
        self.group_nodes();
        Ok(())
    }

    /// The dense indices of the nodes named `source` and `target`, or `None` if either is
    /// not kept. Split out of `admit_edge` so a caller that already
    /// holds resolved endpoints — the columnar path, where an endpoint is a row — pays no
    /// arena probe, and so `index_model` can resolve before it interns anything.
    pub(super) fn endpoints(&self, source: &str, target: &str) -> Option<(u32, u32)> {
        Some((self.node_index(source)?, self.node_index(target)?))
    }

    /// The out, in and hierarchy CSRs, fed in edge order, and the degree column. A
    /// hierarchy edge is filed under its parent, which for `child_of` is its target.
    fn build_adjacency(&mut self) -> Result<(), CapacityError> {
        let (n, e) = (self.node_count(), &self.edges);
        self.out = Csr::from_pairs(n, e.source.iter().copied().zip(0..))?;
        self.inbound = Csr::from_pairs(n, e.target.iter().copied().zip(0..))?;
        let tree = (0..self.edge_count()).filter(|&i| e.kind[i as usize] == EdgeKind::Hierarchy);
        self.hierarchy = Csr::from_pairs(n, tree.map(|i| (self.parent(i), i)))?;
        self.nodes.degree = (0..n)
            .map(|v| (self.out.row(v).len() + self.inbound.row(v).len()) as u32)
            .collect();
        Ok(())
    }

    /// The group column, `byDatabase` and the note count, in one pass in node order.
    fn group_nodes(&mut self) {
        let n = &mut self.nodes;
        let mut sources = IndexSet::<Interned, FixedState>::default();
        n.group = n
            .source
            .iter()
            .map(|&s| sources.insert_full(s).0 as u32)
            .collect();
        for (i, database) in (0..).zip(&n.database) {
            if let Some(database) = *database {
                self.by_database.entry(database).or_default().push(i);
            }
        }
        self.notes = n.kind.iter().filter(|&&k| k == NodeKind::Note).count() as u32;
    }
}

mod admit;
pub(crate) mod columns;
mod slots;
mod view;

#[cfg(test)]
mod tests;

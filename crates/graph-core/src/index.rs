//! The indexed model (`src/core/model/model.ts`): dense indices over stable ids, the
//! string arena, the SoA columns and three CSR adjacencies, built in one O(n + m) pass.
//!
//! Identity is an insertion-ordered set of interned ids — `IndexSet`, never `HashMap`
//! (H2, D4) — so a node's dense index is the position the oracle's `Map` gives it. The
//! dense index is internal: only the stable string id crosses the wire.

use crate::arena::{CapacityError, FixedState, Interned, StringArena};
use crate::columns::{EdgeColumns, NodeColumns, NodeKind};
use crate::csr::Csr;
use crate::edgekind::EdgeKind;
use crate::records::{EdgeRecord, NodeRecord, NodeView};
use indexmap::{IndexMap, IndexSet};

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
    node_ids: IndexSet<Interned, FixedState>,
    edge_ids: IndexSet<Interned, FixedState>,
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
    let (strings, bytes) = size_hint(nodes, edges);
    let mut topology = Topology {
        strings: StringArena::with_capacity(strings, bytes),
        node_ids: IndexSet::with_capacity_and_hasher(nodes.len(), FixedState::default()),
        edge_ids: IndexSet::with_capacity_and_hasher(edges.len(), FixedState::default()),
        nodes: NodeColumns::with_capacity(nodes.len()),
        edges: EdgeColumns::with_capacity(edges.len()),
        ..Topology::default()
    };
    for node in nodes {
        topology.admit_node(node)?;
    }
    for edge in edges {
        topology.admit_edge(edge)?;
    }
    topology.build_adjacency()?;
    topology.group_nodes();
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

/// A dense index for the next entry of a set already holding `len`, or the refusal.
fn next_index(len: usize, what: &'static str) -> Result<u32, CapacityError> {
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

impl Topology {
    /// Keeps `node` unless its id is taken: first wins (`model.ts:37-40`).
    fn admit_node(&mut self, node: &NodeRecord) -> Result<(), CapacityError> {
        if self.node_index(&node.id).is_some() {
            return Ok(());
        }
        next_index(self.node_ids.len(), "node index")?;
        let s = &mut self.strings;
        let id = s.intern(&node.id)?;
        let n = &mut self.nodes;
        n.database.push(intern_opt(s, node.database_id.as_deref())?);
        n.source.push(s.intern(&node.source)?);
        n.label.push(s.intern(&node.label)?);
        n.group_label.push(intern_opt(s, node.group.as_deref())?);
        n.icon.push(intern_opt(s, node.icon.as_deref())?);
        n.id.push(id);
        n.kind.push(node.kind);
        n.weight.push(node.weight);
        n.version.push(node.version);
        n.has_note.push(node.has_note);
        self.node_ids.insert(id);
        Ok(())
    }

    /// Keeps `edge` unless its id is taken or an endpoint is missing (`model.ts:47-53`).
    /// A dropped edge interns nothing, so it neither claims its id nor costs arena bytes.
    fn admit_edge(&mut self, edge: &EdgeRecord) -> Result<(), CapacityError> {
        if self.edge_index(&edge.id).is_some() {
            return Ok(());
        }
        let (Some(source), Some(target)) =
            (self.node_index(&edge.source), self.node_index(&edge.target))
        else {
            return Ok(());
        };
        next_index(self.edge_ids.len(), "edge index")?;
        let s = &mut self.strings;
        let id = s.intern(&edge.id)?;
        let e = &mut self.edges;
        e.label.push(s.intern(&edge.label)?);
        e.record_id.push(intern_opt(s, edge.record_id.as_deref())?);
        e.id.push(id);
        e.source.push(source);
        e.target.push(target);
        e.kind.push(edge.kind);
        e.strength.push(edge.strength);
        e.directed.push(edge.directed);
        e.child_first.push(edge.child_first);
        self.edge_ids.insert(id);
        Ok(())
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

mod view;

#[cfg(test)]
mod tests;

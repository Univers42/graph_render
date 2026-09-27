//! The indexed model (`src/core/model/model.ts`): dense indices over stable ids, the
//! string arena, the SoA columns and three CSR adjacencies, built in one O(n + m) pass.
//!
//! Identity is an insertion-ordered set of interned ids — `IndexSet`, never `HashMap`
//! (H2, D4) — so a node's dense index is the position the oracle's `Map` gives it. The
//! dense index is internal: only the stable string id crosses the wire.

use crate::arena::{CapacityError, FixedState, Interned, StringArena};
use crate::columns::{EdgeColumns, NodeColumns, NodeKind};
use crate::csr::{Csr, Incident};
use crate::edgekind::EdgeKind;
use crate::records::{EdgeRecord, EdgeView, NodeRecord, NodeView};
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
pub fn index_model(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> Result<Topology, CapacityError> {
    let mut topology = Topology {
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
        self.edge_ids.insert(id);
        Ok(())
    }

    /// The out, in and hierarchy CSRs, fed in edge order, and the degree column.
    fn build_adjacency(&mut self) -> Result<(), CapacityError> {
        let (n, e) = (self.node_count(), &self.edges);
        self.out = Csr::from_pairs(n, e.source.iter().copied().zip(0..))?;
        self.inbound = Csr::from_pairs(n, e.target.iter().copied().zip(0..))?;
        let parents = e.source.iter().copied().zip(0..).zip(e.kind.iter());
        let tree = parents.filter(|(_, kind)| **kind == EdgeKind::Hierarchy);
        self.hierarchy = Csr::from_pairs(n, tree.map(|(pair, _)| pair))?;
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

    /// Kept nodes.
    pub fn node_count(&self) -> u32 {
        self.nodes.id.len() as u32
    }

    /// Kept edges.
    pub fn edge_count(&self) -> u32 {
        self.edges.id.len() as u32
    }

    /// `GraphStats`.
    pub fn stats(&self) -> Stats {
        Stats {
            nodes: self.node_count(),
            edges: self.edge_count(),
            databases: self.by_database.len() as u32,
            notes: self.notes,
        }
    }

    /// The dense index of node `id`, if kept.
    pub fn node_index(&self, id: &str) -> Option<u32> {
        let handle = self.strings.find(id)?;
        self.node_ids.get_index_of(&handle).map(|i| i as u32)
    }

    /// The dense index of edge `id`, if kept.
    pub fn edge_index(&self, id: &str) -> Option<u32> {
        let handle = self.strings.find(id)?;
        self.edge_ids.get_index_of(&handle).map(|i| i as u32)
    }

    /// Node `index`'s fields.
    pub fn node(&self, index: u32) -> NodeView<'_> {
        let (i, n, s) = (index as usize, &self.nodes, &self.strings);
        let text = |h: Option<Interned>| h.map(|h| s.get(h));
        NodeView {
            id: s.get(n.id[i]),
            kind: n.kind[i],
            database_id: text(n.database[i]),
            source: s.get(n.source[i]),
            label: s.get(n.label[i]),
            group: text(n.group_label[i]),
            weight: n.weight[i],
            version: n.version[i],
            has_note: n.has_note[i],
            icon: text(n.icon[i]),
        }
    }

    /// Edge `index`'s fields, endpoints as node ids.
    pub fn edge(&self, index: u32) -> EdgeView<'_> {
        let (i, e, s) = (index as usize, &self.edges, &self.strings);
        EdgeView {
            id: s.get(e.id[i]),
            source: s.get(self.nodes.id[e.source[i] as usize]),
            target: s.get(self.nodes.id[e.target[i] as usize]),
            kind: e.kind[i],
            label: s.get(e.label[i]),
            strength: e.strength[i],
            directed: e.directed[i],
            record_id: e.record_id[i].map(|h| s.get(h)),
        }
    }

    /// The oracle's `adjacency.get(id)` for node `node`: every incident edge in edge
    /// order, a self-loop twice. A merge of the out and in rows, both ascending.
    pub fn incident(&self, node: u32) -> Incident<'_> {
        Incident::merge(self.out.row(node), self.inbound.row(node))
    }

    /// `byDatabase`, in first-seen order: database id and its nodes in node order.
    pub fn by_database(&self) -> impl Iterator<Item = (&str, &[u32])> {
        self.by_database
            .iter()
            .map(|(&h, nodes)| (self.strings.get(h), nodes.as_slice()))
    }

    /// The string arena.
    pub fn strings(&self) -> &StringArena {
        &self.strings
    }

    /// Node columns.
    pub fn nodes(&self) -> &NodeColumns {
        &self.nodes
    }

    /// Edge columns.
    pub fn edges(&self) -> &EdgeColumns {
        &self.edges
    }

    /// Node → edges it is the source of, ascending.
    pub fn out(&self) -> &Csr {
        &self.out
    }

    /// Node → edges it is the target of, ascending.
    pub fn inbound(&self) -> &Csr {
        &self.inbound
    }

    /// Source → its `hierarchy` edges, ascending. Read as parent → children.
    ///
    /// Ponytail: orientation. `child_of` classifies as `Hierarchy` too, and there the
    /// source is the child, so an A→B `child_of` edge lands in row A and the tree is
    /// silently inverted for it (wrong result, no error). Nothing reads this CSR yet;
    /// Phase 3, its first reader, must decide the orientation of `child_of` first.
    pub fn hierarchy(&self) -> &Csr {
        &self.hierarchy
    }
}

#[cfg(test)]
mod tests;

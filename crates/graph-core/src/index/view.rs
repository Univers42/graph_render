//! Reading a built [`Topology`]: counts, lookups, one node or edge as a view, and the
//! adjacencies. Split from `index.rs` for the house line limit; the fields are the
//! parent module's.

use super::{Stats, Topology};
use crate::arena::{Interned, StringArena};
use crate::columns::{EdgeColumns, NodeColumns};
use crate::csr::{Csr, Incident};
use crate::records::{EdgeView, NodeView};

impl Topology {
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
            child_first: e.child_first[i],
        }
    }

    /// The parent end of edge `e`: its target when the wire named the child first
    /// (`child_of`), else its source. Only hierarchy edges are read this way.
    pub fn parent(&self, e: u32) -> u32 {
        let i = e as usize;
        if self.edges.child_first[i] {
            self.edges.target[i]
        } else {
            self.edges.source[i]
        }
    }

    /// The child end of edge `e`: the end [`parent`](Self::parent) is not.
    pub fn child(&self, e: u32) -> u32 {
        let i = e as usize;
        if self.edges.child_first[i] {
            self.edges.source[i]
        } else {
            self.edges.target[i]
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

    /// Parent → its `hierarchy` edges, ascending: row `p` holds every hierarchy edge
    /// whose [`parent`](Self::parent) is `p`, so a `child_of` edge sits in its target's
    /// row. The values are edge indices; the child of each is [`child`](Self::child),
    /// never `target` read directly.
    pub fn hierarchy(&self) -> &Csr {
        &self.hierarchy
    }
}

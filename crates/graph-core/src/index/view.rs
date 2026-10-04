//! Reading a built [`Topology`]: counts, lookups, one node or edge as a view, and the
//! adjacencies. Split from `index.rs` for the house line limit; the fields are the
//! parent module's.

use super::{Stats, Topology};
use crate::arena::{Interned, StringArena};
use crate::columns::{EdgeColumns, NodeColumns};
use crate::csr::{AppendCsr, Incident};
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
        self.node_ids.row(handle)
    }

    /// The dense index of edge `id`, if kept.
    pub fn edge_index(&self, id: &str) -> Option<u32> {
        let handle = self.strings.find(id)?;
        self.edge_ids.row(handle)
    }

    /// Node `index`'s fields.
    ///
    /// # Precondition
    ///
    /// `index < self.node_count()`. This indexes the columns directly and **panics** on an
    /// out-of-range `index`; the same reasoning as [`parent`](Self::parent) applies, and
    /// `empty_model()` — a public constructor — makes the empty case reachable from safe
    /// code. The `Option`-returning counterparts are
    /// [`node_index`](Self::node_index) and [`edge_index`](Self::edge_index).
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

    /// Node `index`'s id: [`node`](Self::node)'s first field, alone. Ten fields and five
    /// arena lookups is what a reader of the whole node pays; `layout::snapshot` reads
    /// only the id, at a million of them, so it asks for this instead.
    pub fn node_id(&self, index: u32) -> &str {
        self.strings.get(self.nodes.id[index as usize])
    }

    /// Edge `index`'s id: [`edge`](Self::edge)'s first field, alone — no endpoints.
    pub fn edge_id(&self, index: u32) -> &str {
        self.strings.get(self.edges.id[index as usize])
    }

    /// Edge `index`'s fields, endpoints as node ids.
    ///
    /// # Precondition
    ///
    /// `index < self.edge_count()`, exactly as for [`node`](Self::node); the same
    /// reasoning, and the same panic, apply.
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
    ///
    /// # Precondition
    ///
    /// `e < self.edge_count()`. Unlike the `Option`-returning accessors beside it
    /// ([`node_index`](Self::node_index), [`edge_index`](Self::edge_index)), this indexes
    /// the column directly and **panics** on an out-of-range `e` rather than returning
    /// `None`. The distinction is deliberate: every call site reads `e` out of a CSR row
    /// or a `0..edge_count()` walk — `self.hierarchy()`, `self.out()`,
    /// `self.inbound()` — which is valid by construction, so the checked form would only
    /// add an `expect` saying "this cannot happen" at each of them. An out-of-range index
    /// is a caller bug, not a runtime condition, and panicking on one beats silently
    /// reading an adjacent edge's endpoints.
    pub fn parent(&self, e: u32) -> u32 {
        let i = e as usize;
        if self.edges.child_first[i] {
            self.edges.target[i]
        } else {
            self.edges.source[i]
        }
    }

    /// The child end of edge `e`: the end [`parent`](Self::parent) is not.
    ///
    /// # Precondition
    ///
    /// `e < self.edge_count()`, exactly as for [`parent`](Self::parent); the same
    /// reasoning, and the same panic, apply.
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
    ///
    /// # Precondition
    ///
    /// `node < self.node_count()` — the rows are read by index, so an out-of-range `node`
    /// panics inside [`Csr::row`] rather than answering an empty adjacency; see
    /// [`node`](Self::node). Both rows ascending, which [`Csr::from_pairs`] does not
    /// promise and [`Incident::merge`] asserts in a debug build.
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
    pub fn out(&self) -> &AppendCsr {
        &self.out
    }

    /// Node → edges it is the target of, ascending.
    pub fn inbound(&self) -> &AppendCsr {
        &self.inbound
    }

    /// Parent → its `hierarchy` edges, ascending: row `p` holds every hierarchy edge
    /// whose [`parent`](Self::parent) is `p`, so a `child_of` edge sits in its target's
    /// row. The values are edge indices; the child of each is [`child`](Self::child),
    /// never `target` read directly.
    pub fn hierarchy(&self) -> &AppendCsr {
        &self.hierarchy
    }
}

#[cfg(test)]
mod tests;

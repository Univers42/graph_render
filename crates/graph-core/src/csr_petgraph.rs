//! petgraph trait impls over `Topology`'s own CSR (Phase 7, `prompts/phase-07-analysis.md`
//! step 1): reuse petgraph's algorithms without a second topology representation. No
//! `petgraph::Graph` is built here — every trait method reads `Topology`'s existing
//! columns and CSRs directly, so this file is trait impls only, no algorithms.
//!
//! Mixed edges: `edge.directed` decides traversal — directed is source→target only,
//! undirected is both ways (so it also appears reversed). Forward edges from `v` are
//! `out`'s arrival order then `inbound`'s undirected edges — the CSR order Phase 1
//! matched to the oracle's `Map` (H1), so every analysis here inherits it for free.

use crate::columns::EdgeColumns;
use crate::index::Topology;
use petgraph::visit::{
    Data, GraphBase, GraphRef, IntoEdgeReferences, IntoEdges, IntoNeighbors, IntoNodeIdentifiers,
    NodeCompactIndexable, NodeCount, NodeIndexable, VisitMap, Visitable,
};
use std::iter::Map;
use std::ops::Range;
use std::slice;

/// A node, as petgraph sees it: the dense index, nothing more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeIx(pub u32);

/// An edge, as petgraph sees it: its dense index into the edge columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeIx(pub u32);

/// One edge, with the fields `petgraph::visit::EdgeRef` needs: reads straight off the
/// edge columns, nothing copied but the four scalars.
#[derive(Debug, Clone, Copy)]
pub struct CsrEdgeRef {
    source: NodeIx,
    target: NodeIx,
    weight: f64,
    id: EdgeIx,
}

impl petgraph::visit::EdgeRef for CsrEdgeRef {
    type NodeId = NodeIx;
    type EdgeId = EdgeIx;
    type Weight = f64;
    fn source(&self) -> NodeIx {
        self.source
    }
    fn target(&self) -> NodeIx {
        self.target
    }
    fn weight(&self) -> &f64 {
        &self.weight
    }
    fn id(&self) -> EdgeIx {
        self.id
    }
}

/// `Topology`, as petgraph sees it: directed, weighted by `strength`. Copy: it is a
/// borrow of the topology and nothing else, cheap to pass by value as every `Into-`
/// trait here expects.
#[derive(Debug, Clone, Copy)]
pub struct CsrDigraph<'a> {
    topology: &'a Topology,
}

impl<'a> CsrDigraph<'a> {
    /// Wraps `topology`. Borrows only; nothing is copied.
    pub fn new(topology: &'a Topology) -> Self {
        Self { topology }
    }

    /// `v`'s forward edges.
    fn forward_edges(self, v: u32) -> ForwardEdges<'a> {
        ForwardEdges {
            edges: self.topology.edges(),
            source: v,
            out: self.topology.out().row(v).iter(),
            back: self.topology.inbound().row(v).iter(),
        }
    }
}

/// `v`'s forward edges: `out`'s arrival order, then `inbound`'s edges whose
/// `directed` flag is false (an undirected edge is traversable from either end).
pub struct ForwardEdges<'a> {
    edges: &'a EdgeColumns,
    source: u32,
    out: slice::Iter<'a, u32>,
    back: slice::Iter<'a, u32>,
}

impl ForwardEdges<'_> {
    /// Edge `id`'s ref, from this node, to `target` — `out` and `back` each resolve a
    /// different column for `target`, everything else is shared.
    fn make(&self, id: u32, target: u32) -> CsrEdgeRef {
        let (source, weight) = (NodeIx(self.source), self.edges.strength[id as usize]);
        CsrEdgeRef {
            source,
            target: NodeIx(target),
            weight,
            id: EdgeIx(id),
        }
    }
}

impl Iterator for ForwardEdges<'_> {
    type Item = CsrEdgeRef;

    fn next(&mut self) -> Option<CsrEdgeRef> {
        if let Some(&e) = self.out.next() {
            return Some(self.make(e, self.edges.target[e as usize]));
        }
        while let Some(&e) = self.back.next() {
            if !self.edges.directed[e as usize] {
                return Some(self.make(e, self.edges.source[e as usize]));
            }
        }
        None
    }
}

fn edge_target(e: CsrEdgeRef) -> NodeIx {
    e.target
}

/// Every edge, source-then-target exactly as the columns hold it, direction ignored —
/// what a weak-connectivity check needs (`components.rs`).
pub struct AllEdges<'a> {
    edges: &'a EdgeColumns,
    next: u32,
    len: u32,
}

impl Iterator for AllEdges<'_> {
    type Item = CsrEdgeRef;

    fn next(&mut self) -> Option<CsrEdgeRef> {
        if self.next >= self.len {
            return None;
        }
        let i = self.next as usize;
        self.next += 1;
        Some(CsrEdgeRef {
            source: NodeIx(self.edges.source[i]),
            target: NodeIx(self.edges.target[i]),
            weight: self.edges.strength[i],
            id: EdgeIx(i as u32),
        })
    }
}

/// `0..node_count()`, forward — the fixed visit order every analysis result depends on.
pub struct NodeIds(Range<u32>);

impl Iterator for NodeIds {
    type Item = NodeIx;
    fn next(&mut self) -> Option<NodeIx> {
        self.0.next().map(NodeIx)
    }
}

/// A visited set over dense indices, backing [`Visitable`] — a `Vec<bool>`, not a
/// `HashSet`: D4 does not apply (membership only, order never read).
pub struct VisitedSet(Vec<bool>);

impl VisitMap<NodeIx> for VisitedSet {
    fn visit(&mut self, a: NodeIx) -> bool {
        let slot = &mut self.0[a.0 as usize];
        let first = !*slot;
        *slot = true;
        first
    }
    fn is_visited(&self, a: &NodeIx) -> bool {
        self.0[a.0 as usize]
    }
    fn unvisit(&mut self, a: NodeIx) -> bool {
        let slot = &mut self.0[a.0 as usize];
        let was = *slot;
        *slot = false;
        was
    }
}

impl GraphBase for CsrDigraph<'_> {
    type NodeId = NodeIx;
    type EdgeId = EdgeIx;
}
impl GraphRef for CsrDigraph<'_> {}
impl Data for CsrDigraph<'_> {
    type NodeWeight = ();
    type EdgeWeight = f64;
}

impl<'a> IntoNodeIdentifiers for CsrDigraph<'a> {
    type NodeIdentifiers = NodeIds;
    fn node_identifiers(self) -> NodeIds {
        NodeIds(0..self.topology.node_count())
    }
}
impl<'a> IntoNeighbors for CsrDigraph<'a> {
    type Neighbors = Map<ForwardEdges<'a>, fn(CsrEdgeRef) -> NodeIx>;
    fn neighbors(self, a: NodeIx) -> Self::Neighbors {
        self.forward_edges(a.0).map(edge_target)
    }
}
impl<'a> IntoEdgeReferences for CsrDigraph<'a> {
    type EdgeRef = CsrEdgeRef;
    type EdgeReferences = AllEdges<'a>;
    fn edge_references(self) -> AllEdges<'a> {
        AllEdges {
            edges: self.topology.edges(),
            next: 0,
            len: self.topology.edge_count(),
        }
    }
}
impl<'a> IntoEdges for CsrDigraph<'a> {
    type Edges = ForwardEdges<'a>;
    fn edges(self, a: NodeIx) -> ForwardEdges<'a> {
        self.forward_edges(a.0)
    }
}

impl NodeCount for CsrDigraph<'_> {
    fn node_count(&self) -> usize {
        self.topology.node_count() as usize
    }
}
impl NodeIndexable for CsrDigraph<'_> {
    fn node_bound(&self) -> usize {
        self.topology.node_count() as usize
    }
    fn to_index(&self, a: NodeIx) -> usize {
        a.0 as usize
    }
    fn from_index(&self, i: usize) -> NodeIx {
        // `NodeCompactIndexable` promises one node per index below `node_bound()`, which
        // is the node count, so `i < u32::MAX` holds for every index this graph hands
        // petgraph. `expect`, not `as u32`: a silent truncation would answer a different
        // node — `from_index(2³²) == NodeIx(0)` reads node 0's adjacency — and a reindexing
        // algorithm would never know.
        NodeIx(u32::try_from(i).expect("node index exceeds u32"))
    }
}
impl NodeCompactIndexable for CsrDigraph<'_> {}
impl Visitable for CsrDigraph<'_> {
    type Map = VisitedSet;
    fn visit_map(&self) -> VisitedSet {
        VisitedSet(vec![false; self.topology.node_count() as usize])
    }
    fn reset_map(&self, map: &mut VisitedSet) {
        map.0.clear();
        map.0.resize(self.topology.node_count() as usize, false);
    }
}

#[cfg(test)]
mod tests;

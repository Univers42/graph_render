//! The grid as a petgraph graph: the trait impls that let Phase 7's Dijkstra run over
//! [`super::csr::GridCsr`], and nothing else.
//!
//! Split out of `csr.rs` for the house's 300-line limit, and because it is a different
//! concern: `csr.rs` builds the adjacency, this file only *exposes* it. No algorithm lives
//! in either — the shortest path is petgraph's, called once from
//! [`crate::post::routed::route`].
//!
//! The one thing worth knowing before editing: **every impl is on the owned
//! [`GridGraph`]**, never split between `GridCsr` and `&GridCsr`. `dijkstra` requires
//! `IntoEdges + Visitable` on *one* type, and a split satisfies neither combination while
//! still compiling — into a search that never leaves its source. That bug was real here and
//! is recorded in `docs/measurements/phase08-routing.md` §3.

use super::csr::{Cell, GridCsr};
use petgraph::visit::{
    Data, EdgeRef, GraphBase, GraphRef, IntoEdgeReferences, IntoEdges, IntoNeighbors, VisitMap,
    Visitable,
};
use std::iter::Empty;

/// One step of the grid graph: cell `source` to cell `target`, costing the stencil step's
/// Euclidean length.
///
/// The *occupancy* rule is not here: an occupied cell is not "expensive", it is
/// impassable, and which two cells are an edge's own endpoints is the query's business
/// ([`crate::post::routed::route`]), not the graph's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GridEdge {
    index: u32,
    source: Cell,
    target: Cell,
    cost: f64,
}

impl EdgeRef for GridEdge {
    type NodeId = Cell;
    type EdgeId = u32;
    type Weight = f64;

    fn source(&self) -> Cell {
        self.source
    }

    fn target(&self) -> Cell {
        self.target
    }

    fn weight(&self) -> &f64 {
        &self.cost
    }

    fn id(&self) -> u32 {
        self.index
    }
}

/// The visit map Dijkstra marks settled cells in: one bit per cell, reused across
/// queries by whoever owns the graph. `Vec<bool>` rather than a hash set, so a settled
/// cell is a load and not a hash probe.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Visited(Vec<bool>);

impl Visited {
    /// A map over `cells` cells, all unvisited.
    pub fn over(cells: u32) -> Self {
        Self(vec![false; cells as usize])
    }

    /// Marks every cell unvisited, keeping the allocation — the reuse `dsa-and-memory.md`
    /// asks for, since Dijkstra allocates this once per query otherwise.
    pub fn clear(&mut self) {
        self.0.fill(false);
    }
}

impl VisitMap<Cell> for Visited {
    fn visit(&mut self, a: Cell) -> bool {
        let slot = self.0.get_mut(a as usize);
        match slot {
            Some(seen) if *seen => false,
            Some(seen) => {
                *seen = true;
                true
            }
            None => false,
        }
    }

    fn is_visited(&self, a: &Cell) -> bool {
        self.0.get(*a as usize).copied().unwrap_or(false)
    }

    fn unvisit(&mut self, a: Cell) -> bool {
        match self.0.get_mut(a as usize) {
            Some(seen) if *seen => {
                *seen = false;
                true
            }
            _ => false,
        }
    }
}

/// `cell`'s outgoing edges, in [`super::csr::STENCIL`] order — a row read through the CSR,
/// no per-edge allocation.
#[derive(Debug, Clone)]
pub struct GridEdges<'a> {
    costs: &'a [f64; 8],
    /// The row, copied: at most 8 entries, and it lets the iterator outlive the borrow of
    /// the graph that produced it, which is what petgraph's `IntoEdges` signature needs.
    values: Vec<Cell>,
    source: Cell,
    slot: usize,
}

impl Iterator for GridEdges<'_> {
    type Item = GridEdge;

    fn next(&mut self) -> Option<GridEdge> {
        let target = *self.values.get(self.slot)?;
        let cost = self.costs[self.slot];
        let index = self.source * 8 + u32::try_from(self.slot).expect("8 slots fit u32");
        self.slot += 1;
        Some(GridEdge {
            index,
            source: self.source,
            target,
            cost,
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = self.values.len() - self.slot;
        (left, Some(left))
    }
}

/// The grid graph as petgraph sees it: a `Copy` handle on a borrowed [`GridCsr`], with no
/// container of its own. Phase 7's `CsrDigraph` shape (`csr_petgraph.rs`), and the only one
/// that works: `dijkstra` needs `IntoEdges + Visitable` satisfied by **one** type, so
/// splitting the impls between `GridCsr` and `&GridCsr` satisfies neither combination and
/// silently compiles into a search that never leaves its source.
#[derive(Debug, Clone, Copy)]
pub struct GridGraph<'a> {
    graph: &'a GridCsr,
}

impl<'a> GridGraph<'a> {
    /// Borrows `graph`; nothing is copied.
    pub fn new(graph: &'a GridCsr) -> Self {
        Self { graph }
    }
}

impl GraphBase for GridGraph<'_> {
    type NodeId = Cell;
    type EdgeId = u32;
}

impl GraphRef for GridGraph<'_> {}

/// The grid graph carries no node payload: a cell is its own dense index and holds nothing
/// a route reads but its occupancy, which lives in the grid index the weight closure in
/// [`crate::post::routed::route`] consults. `()` for the node weight, so none is copied.
impl Data for GridGraph<'_> {
    type NodeWeight = ();
    type EdgeWeight = f64;
}

impl<'a> IntoEdgeReferences for GridGraph<'a> {
    type EdgeRef = GridEdge;
    type EdgeReferences = Empty<GridEdge>;

    /// Empty: `dijkstra` walks `edges(a)` per node and never asks for the whole edge set.
    /// Present only because `IntoEdges` requires it.
    fn edge_references(self) -> Self::EdgeReferences {
        std::iter::empty()
    }
}

impl<'a> IntoNeighbors for GridGraph<'a> {
    type Neighbors = std::vec::IntoIter<Cell>;

    fn neighbors(self, a: Cell) -> Self::Neighbors {
        self.graph
            .row(a)
            .map(|(_, v)| v)
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl<'a> IntoEdges for GridGraph<'a> {
    type Edges = GridEdges<'a>;

    fn edges(self, a: Cell) -> Self::Edges {
        let targets = self.graph.targets(a);
        GridEdges {
            costs: self.graph.costs(),
            values: targets,
            source: a,
            slot: 0,
        }
    }
}

impl Visitable for GridGraph<'_> {
    type Map = Visited;

    fn visit_map(&self) -> Visited {
        Visited::over(self.graph.cells())
    }

    fn reset_map(&self, map: &mut Visited) {
        map.clear();
    }
}

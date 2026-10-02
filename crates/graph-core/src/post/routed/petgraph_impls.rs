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

/// `cell`'s outgoing edges, in [`super::csr::STENCIL`] order: the CSR row, borrowed, so a
/// query allocates nothing per node. Each edge is priced by its own stencil slot, decoded
/// from the step, never by its position in a (possibly shorter, border) row.
#[derive(Debug, Clone)]
pub struct GridEdges<'a> {
    graph: &'a GridCsr,
    targets: std::slice::Iter<'a, Cell>,
    source: Cell,
    at: (i32, i32),
}

impl<'a> GridEdges<'a> {
    fn new(graph: &'a GridCsr, source: Cell) -> Self {
        Self {
            graph,
            targets: graph.targets(source).iter(),
            source,
            at: graph.xy(source),
        }
    }
}

impl Iterator for GridEdges<'_> {
    type Item = GridEdge;

    /// The id is `source · 8 + slot`: unique, and it names the edge's source and stencil
    /// slot. It fits u32 because `GridIndex::build` refuses more than `u32::MAX / 8` cells.
    fn next(&mut self) -> Option<GridEdge> {
        let target = *self.targets.next()?;
        let slot = self.graph.slot_from(self.at, target);
        Some(GridEdge {
            index: self.source * 8 + slot as u32,
            source: self.source,
            target,
            cost: self.graph.cost(slot),
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.targets.size_hint()
    }
}

/// Every edge of the grid: each cell's [`GridEdges`] in cell order, so the same edges, ids
/// and weights `edges(a)` yields, concatenated.
#[derive(Debug, Clone)]
pub struct AllEdges<'a> {
    graph: &'a GridCsr,
    cell: Cell,
    row: GridEdges<'a>,
}

impl Iterator for AllEdges<'_> {
    type Item = GridEdge;

    fn next(&mut self) -> Option<GridEdge> {
        loop {
            if let Some(edge) = self.row.next() {
                return Some(edge);
            }
            if !self.graph.contains(self.cell + 1) {
                return None;
            }
            self.cell += 1;
            self.row = GridEdges::new(self.graph, self.cell);
        }
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
    type EdgeReferences = AllEdges<'a>;

    /// Every edge, cell by cell. `dijkstra` never asks for it, but an algorithm that does
    /// (a spanning tree, an edge count) must see the real edge set, not an empty one.
    fn edge_references(self) -> Self::EdgeReferences {
        AllEdges {
            graph: self.graph,
            cell: 0,
            row: GridEdges::new(self.graph, 0),
        }
    }
}

impl<'a> IntoNeighbors for GridGraph<'a> {
    type Neighbors = std::iter::Copied<std::slice::Iter<'a, Cell>>;

    fn neighbors(self, a: Cell) -> Self::Neighbors {
        self.graph.targets(a).iter().copied()
    }
}

impl<'a> IntoEdges for GridGraph<'a> {
    type Edges = GridEdges<'a>;

    fn edges(self, a: Cell) -> Self::Edges {
        GridEdges::new(self.graph, a)
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

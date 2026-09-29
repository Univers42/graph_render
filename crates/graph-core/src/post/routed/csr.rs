//! The grid as a CSR graph, and the petgraph trait impls that let Phase 7's Dijkstra run
//! over it (`prompts/phase-08-post-routing-bundling.md` step 3: "The grid is a graph; our
//! CSR and Dijkstra already handle graphs").
//!
//! This file is a **builder and trait impls, no algorithms** — the rule Phase 7's
//! `csr_petgraph.rs` sets. No `petgraph::Graph`, `StableGraph` or `GraphMap` is ever
//! built: the adjacency is Phase 1's own [`crate::csr::Csr`] over the grid's own cell
//! numbering, and every trait method reads it directly. That is what keeps one
//! representation of the graph in the crate
//! (`docs/decisions/petgraph-determinism-audit.md`).
//!
//! # The stencil
//!
//! Eight-connected, in the reference's order: `itertools.product((-1, 0, 1), repeat=2)`
//! with the origin removed (`SciGraphs/engine/scigraphs_engine/bundling/routed.py`,
//! `_offsets`). A step costs its Euclidean length — 1 along an axis, `√2` on a diagonal —
//! so the metric is 8% off Euclidean on a diagonal at worst, as the reference records;
//! 4- or 6-connectivity would make it Manhattan. Only [`libm::sqrt`] is called (D1), so
//! the diagonal costs bit-alike on every target.
//!
//! The stencil order is also the CSR row order, so a row is walked in the reference's
//! order and the CSR stays arrival-ordered and deterministic.

use crate::csr::Csr;
use crate::post::grid_index::GridIndex;
use petgraph::visit::{
    Data, EdgeRef, GraphBase, GraphRef, IntoEdgeReferences, IntoEdges, IntoNeighbors, VisitMap,
    Visitable,
};
use std::iter::Empty;

/// The 8-neighbour stencil, `(dx, dy)`, in the reference's `itertools.product` order with
/// the origin removed. Fixed, so a CSR row is always in this order.
pub const STENCIL: [(i32, i32); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

/// A cell of the grid, as petgraph sees it: the dense cell index, nothing more.
pub type Cell = u32;

/// Euclidean length of each stencil step, in [`STENCIL`] order: 1 along an axis, `√2` on
/// a diagonal. `libm::sqrt` only (D1), computed once per grid.
pub fn stencil_costs() -> [f64; 8] {
    let diagonal = libm::sqrt(2.0);
    [
        diagonal,
        1.0,
        diagonal,
        1.0,
        1.0,
        diagonal,
        1.0,
        diagonal,
    ]
}

/// The grid's cells as a weighted CSR graph, in the grid's own cell numbering.
///
/// Node `c` is cell `c`; `offsets` has `cells + 1` rows and `values` one target per
/// in-range neighbour, in [`STENCIL`] order. A row holds no cell outside the grid — there
/// is no wrap, and the border simply has fewer neighbours.
#[derive(Debug, Clone, PartialEq)]
pub struct GridCsr {
    adj: Csr,
    costs: [f64; 8],
    nx: u32,
    cells: u32,
}

/// Builds the CSR over `grid`'s cells: O(cells), eight neighbour tests per cell.
pub fn build_csr(grid: &GridIndex) -> GridCsr {
    let (nx, ny) = grid.shape();
    let cells = grid.cells();
    // `from_pairs` walks the pairs twice, so the iterator must be `Clone`: a
    // `Vec<(u32, u32)>` of at most 8 cells' worth, built once and replayed. The alternative
    // — a hand-rolled counting sort — would duplicate `Csr::from_pairs` for no gain.
    let pairs: Vec<(Cell, Cell)> = (0..cells)
        .flat_map(|cell| neighbours(cell, nx, ny).map(move |(_, v)| (cell, v)))
        .collect();
    let pairs = pairs.into_iter();
    GridCsr {
        adj: Csr::from_pairs(cells, pairs).expect("a grid's rows fit u32 by construction"),
        costs: stencil_costs(),
        nx,
        cells,
    }
}

impl GridCsr {
    /// Number of cells, which is also the node count.
    pub fn cells(&self) -> u32 {
        self.cells
    }

    /// `cell`'s neighbours as `(stencil index, cell)`, in [`STENCIL`] order.
    pub fn row(&self, cell: u32) -> impl Iterator<Item = (usize, Cell)> + '_ {
        self.adj.row(cell).iter().enumerate().map(|(slot, v)| (slot, *v))
    }

    /// Euclidean step length of stencil entry `slot`.
    pub fn cost(&self, slot: usize) -> f64 {
        self.costs[slot]
    }

    /// `true` when `cell` is in range.
    pub fn contains(&self, cell: u32) -> bool {
        cell < self.cells
    }

    /// `(x, y)` of `cell` in cell coordinates, x fastest.
    pub fn xy(&self, cell: u32) -> (i32, i32) {
        ((cell % self.nx) as i32, (cell / self.nx) as i32)
    }
}

/// `cell`'s in-range neighbours as `(stencil index, cell)`, in stencil order.
fn neighbours(cell: u32, nx: u32, ny: u32) -> impl Iterator<Item = (usize, Cell)> {
    let (cx, cy) = (cell % nx, cell / nx);
    let (w, h) = (nx, ny);
    STENCIL
        .iter()
        .enumerate()
        .filter(move |(_, (dx, dy))| {
            inside(cx, *dx, w) && inside(cy, *dy, h)
        })
        .map(move |(slot, (dx, dy))| {
            let x = cx.wrapping_add_signed(*dx);
            let y = cy.wrapping_add_signed(*dy);
            (slot, y * w + x)
        })
}

/// `true` when `delta` steps from `at` stay inside `0..limit`. Written on unsigned
/// counters so the closure above borrows nothing local.
fn inside(at: u32, delta: i32, limit: u32) -> bool {
    let moved = at as i64 + i64::from(delta);
    moved >= 0 && moved < i64::from(limit)
}

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
/// a route reads but its occupancy, which lives in the [`GridIndex`] the weight closure in
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
        self.graph.adj.row(a).to_vec().into_iter()
    }
}

impl<'a> IntoEdges for GridGraph<'a> {
    type Edges = GridEdges<'a>;

    fn edges(self, a: Cell) -> Self::Edges {
        GridEdges {
            costs: &self.graph.costs,
            values: self.graph.adj.row(a),
            source: a,
            slot: 0,
        }
    }
}

/// `cell`'s outgoing edges, in [`STENCIL`] order — a row read through the CSR, no copy.
#[derive(Debug, Clone)]
pub struct GridEdges<'a> {
    costs: &'a [f64; 8],
    values: &'a [Cell],
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

impl Visitable for GridGraph<'_> {
    type Map = Visited;

    fn visit_map(&self) -> Visited {
        Visited::over(self.graph.cells)
    }

    fn reset_map(&self, map: &mut Visited) {
        map.clear();
    }
}

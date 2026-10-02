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
//! 4- or 6-connectivity would make it Manhattan. Only [`f64::sqrt`] is called, which
//! IEEE-754 rounds correctly, so the diagonal costs bit-alike on every target.
//!
//! The stencil order is also the CSR row order, so a row is walked in the reference's
//! order and the CSR stays arrival-ordered and deterministic. A border row is shorter than
//! eight, so a position in the row is **not** a stencil index: [`GridCsr::row`] decodes the
//! index from the step itself, and every price is read through it (review finding R1).

use crate::csr::Csr;
use crate::post::grid_index::GridIndex;

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
/// a diagonal. `f64::sqrt` only, computed once per grid.
///
/// f64, not the reference's float32-rounded literals (`routed.py:69-71`). The reference
/// rounds so that its numpy and GPU backends break a tie alike; here every target computes
/// these bits through libm, and a tie is broken explicitly by cell index ([`super::trace`]).
/// Rounding to f32 would shift √2 by 2.4e-8 and could flip a near-tie between two paths,
/// so the choice is kept and named here (review finding M20).
pub fn stencil_costs() -> [f64; 8] {
    let diagonal = f64::sqrt(2.0);
    [diagonal, 1.0, diagonal, 1.0, 1.0, diagonal, 1.0, diagonal]
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

    /// `cell`'s neighbours as `(stencil index, cell)`, in [`STENCIL`] order. The index is
    /// decoded from the step, never taken from the position in the row; empty for a cell
    /// out of range.
    pub fn row(&self, cell: u32) -> impl Iterator<Item = (usize, Cell)> + '_ {
        let at = self.xy(cell);
        self.targets(cell)
            .iter()
            .map(move |&next| (self.slot_from(at, next), next))
    }

    /// The [`STENCIL`] index of the step from the cell at `from` (its [`Self::xy`]) to its
    /// neighbour `to`.
    pub fn slot_from(&self, from: (i32, i32), to: Cell) -> usize {
        let (x, y) = self.xy(to);
        stencil_slot(x - from.0, y - from.1)
    }

    /// Euclidean step length of stencil entry `slot`.
    pub fn cost(&self, slot: usize) -> f64 {
        self.costs[slot]
    }

    /// `true` when `cell` is in range.
    pub fn contains(&self, cell: u32) -> bool {
        cell < self.cells
    }

    /// `(x, y)` of `cell` in cell coordinates, x fastest. `(0, 0)` on an empty grid, as
    /// [`GridIndex::xy`] answers.
    pub fn xy(&self, cell: u32) -> (i32, i32) {
        if self.nx == 0 {
            return (0, 0);
        }
        ((cell % self.nx) as i32, (cell / self.nx) as i32)
    }

    /// `cell`'s neighbours, the CSR row itself, borrowed, in [`STENCIL`] order: at most 8
    /// entries, and empty for a cell out of range.
    pub fn targets(&self, cell: u32) -> &[Cell] {
        if self.contains(cell) {
            self.adj.row(cell)
        } else {
            &[]
        }
    }
}

/// The [`STENCIL`] index of the step `(dx, dy)`: the product order puts it at
/// `3·(dx + 1) + (dy + 1)`, one less past the removed origin at 4.
fn stencil_slot(dx: i32, dy: i32) -> usize {
    debug_assert!(dx.abs() <= 1 && dy.abs() <= 1 && (dx, dy) != (0, 0));
    let at = (3 * (dx + 1) + (dy + 1)) as usize;
    at - usize::from(at > 4)
}

/// `cell`'s in-range neighbours as `(stencil index, cell)`, in stencil order.
fn neighbours(cell: u32, nx: u32, ny: u32) -> impl Iterator<Item = (usize, Cell)> {
    let (cx, cy) = (cell % nx, cell / nx);
    let (w, h) = (nx, ny);
    STENCIL
        .iter()
        .enumerate()
        .filter(move |(_, (dx, dy))| inside(cx, *dx, w) && inside(cy, *dy, h))
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

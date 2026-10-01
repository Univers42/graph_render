//! The preorder arena a build flattens its pointer tree into. Split from `quadtree.rs` for
//! the house line cap.
//!
//! A walk over pointer nodes chases a child index per visit and keeps a stack of
//! `(node, bounds)` pairs; at 100k nodes that stack was 16% of the tick's instructions
//! (`docs/measurements/perf-p1-baseline.md`). In preorder a cell's subtree is the run
//! `k..skip`, so a pruned walk is `k = skip` and a descent is `k += 1`: no stack, and
//! the cells are read front to back. Each cell's bounds are computed once here, by the
//! same `Bounds::quadrant` from the same root the old walk applied per visit, so every
//! float a pass reads is the one it read before.

use super::{Bounds, Points, Quadtree, Shape};

/// One tree node in preorder, children in slot order `0,1,2,3`.
///
/// `skip` is the index just past this cell's subtree; a cell is a leaf exactly when
/// `skip == k + 1`, since every internal node holds at least one child (`insert_leaf`
/// fills a slot right after pushing an internal node). The subtree's points are
/// `order[start..end]`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cell {
    pub(crate) bounds: Bounds,
    pub(crate) skip: u32,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

/// A pending step of the flatten: enter a pointer node at its bounds, or close cell `k`
/// once its whole subtree has been laid out.
#[derive(Debug, Clone, Copy)]
pub(super) enum Visit {
    Open(u32, Bounds),
    Close(u32),
}

impl Quadtree {
    /// Lays the pointer tree out in preorder, then appends the points no leaf holds (a NaN
    /// coordinate) in ascending index, so `order` is a permutation of every point.
    pub(super) fn flatten(&mut self, pts: Points<'_>) {
        self.cells.clear();
        self.key.clear();
        self.order.clear();
        self.pending.clear();
        self.pending
            .extend(self.root.map(|root| Visit::Open(root, self.root_bounds)));
        while let Some(visit) = self.pending.pop() {
            match visit {
                Visit::Open(node, bounds) => self.open(node, bounds),
                Visit::Close(k) => {
                    let (skip, end) = (self.cells.len() as u32, self.order.len() as u32);
                    let cell = &mut self.cells[k as usize];
                    (cell.skip, cell.end) = (skip, end);
                }
            }
        }
        for i in 0..pts.xs.len() as u32 {
            let (x, y) = pts.at(i);
            if x.is_nan() || y.is_nan() {
                self.order.push(i);
            }
        }
    }

    /// Appends pointer node `node` as the next cell: a leaf with its whole chain, or an
    /// internal cell whose children queue `3,2,1,0` so they lay out `0,1,2,3`.
    fn open(&mut self, node: u32, bounds: Bounds) {
        let k = self.cells.len() as u32;
        let start = self.order.len() as u32;
        self.key.push(node);
        match self.shape[node as usize] {
            Shape::Leaf(head) => {
                let mut point = Some(head);
                while let Some(p) = point {
                    self.order.push(p);
                    point = self.chain_next[p as usize];
                }
                let end = self.order.len() as u32;
                let skip = k + 1;
                self.cells.push(Cell {
                    bounds,
                    skip,
                    start,
                    end,
                });
            }
            Shape::Internal(children) => {
                self.cells.push(Cell {
                    bounds,
                    skip: 0,
                    start,
                    end: 0,
                });
                self.pending.push(Visit::Close(k));
                for slot in (0..4).rev() {
                    if let Some(child) = children[slot] {
                        self.pending.push(Visit::Open(child, bounds.quadrant(slot)));
                    }
                }
            }
        }
    }
}

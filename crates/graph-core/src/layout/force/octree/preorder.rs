//! The preorder arena a build flattens its pointer tree into. Split from `octree.rs` for
//! the house line cap, and structurally identical to `quadtree/preorder.rs`: a cell's
//! subtree is the run `k..skip`, so a pruned walk is `k = skip` and a descent is `k += 1` —
//! no stack, and the cells are read front to back.

use super::{Bounds3, Octree, Points3, Shape, OCTANTS};

/// One tree node in preorder, children in slot order `0..7`.
///
/// `skip` is the index just past this cell's subtree; a cell is a leaf exactly when
/// `skip == k + 1`, since every internal node holds at least one child. The subtree's
/// points are `order[start..end]`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Cell {
    pub(crate) bounds: Bounds3,
    pub(crate) skip: u32,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

/// A pending step of the flatten: enter a pointer node at its bounds, or close cell `k`
/// once its whole subtree has been laid out.
#[derive(Debug, Clone, Copy)]
pub(super) enum Visit {
    Open(u32, Bounds3),
    Close(u32),
}

impl Octree {
    /// Lays the pointer tree out in preorder, then appends the points no leaf holds (a
    /// non-finite coordinate) in ascending index, so `order` is a permutation of every
    /// point. The arena buffers are already clear — [`Octree::reset`](super::Octree)
    /// cleared them — so a refused build never reaches here.
    pub(super) fn flatten(&mut self, pts: Points3<'_>) {
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
        for i in 0..pts.len() as u32 {
            let (x, y, z) = pts.at(i);
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                self.order.push(i);
            }
        }
    }

    /// Appends pointer node `node` as the next cell: a leaf with its whole chain, or an
    /// internal cell whose children queue `7,6,…,0` so they lay out `0,1,…,7`.
    fn open(&mut self, node: u32, bounds: Bounds3) {
        let k = self.cells.len() as u32;
        let start = self.order.len() as u32;
        self.node_id.push(node);
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
                for slot in (0..OCTANTS).rev() {
                    if let Some(child) = children[slot] {
                        self.pending
                            .push(Visit::Open(child, bounds.octant(slot)));
                    }
                }
            }
        }
    }
}
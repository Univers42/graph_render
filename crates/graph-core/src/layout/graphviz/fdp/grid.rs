//! The repulsion grid `fdp` uses instead of the `O(n^2)` all-pairs sweep.
//!
//! The reference stores cells in a `Dtoset` dictionary keyed by the integer pair
//! `(floor(x / Cell), floor(y / Cell))` (`grid.c:143-150,203-215`) and walks it in
//! comparison order, cell by cell. Each cell's node list is built by **prepending**
//! (`grid.c:211`), so within a cell the nodes come out in reverse insertion order, and
//! each cell is followed by its eight neighbours in one fixed order
//! (`tlayout.c:271-278`). All three are reproduced here, because the walk order decides the
//! order of the floating-point additions into the displacement columns and therefore the
//! last bits of the answer.
//!
//! A [`BTreeMap`] over `(i, j)` gives the dictionary's comparison order, and a cell's `Vec`
//! is filled by pushing then reversing, which is the prepend. The cell is `Cell = 3 * K`
//! square (`tlayout.c:177-180`), and only the *neighbour* cells are distance-culled — the
//! within-cell repulsion is unconditional (`tlayout.c:265-269` against `243`).

use std::collections::BTreeMap;

use super::model::Model;
use super::rng::GlibcRand;
use super::{CELL, K};

/// A cell index: `floor` of the position over the cell size, on each axis.
type Key = (i32, i32);

/// The eight neighbour offsets, in the reference's own order (`tlayout.c:271-278`). The
/// order is part of the answer: it is the order the additions land in.
const NEIGHBOURS: [(i32, i32); 8] = [
    (-1, -1),
    (-1, 0),
    (-1, 1),
    (0, -1),
    (0, 1),
    (1, -1),
    (1, 0),
    (1, 1),
];

/// The repulsion grid for one tick. Rebuilt from scratch by [`Grid::fill`].
pub(super) struct Grid {
    cells: BTreeMap<Key, Vec<u32>>,
}

impl Grid {
    /// An empty grid. The reference allocates its cell free list once and clears it per
    /// tick (`grid.c:186-192`); a fresh map per tick is the same thing without the free
    /// list, and it cannot carry a stale cell across a tick.
    pub(super) fn new() -> Self {
        Self {
            cells: BTreeMap::new(),
        }
    }

    /// Put every node in the cell its position falls in, then reverse each cell's list so
    /// the walk sees the reference's prepend order.
    pub(super) fn fill(&mut self, model: &Model) {
        self.cells.clear();
        for i in 0..model.count as usize {
            let key = (cell_of(model.x[i]), cell_of(model.y[i]));
            self.cells.entry(key).or_default().push(i as u32);
        }
        for nodes in self.cells.values_mut() {
            nodes.reverse();
        }
    }

    /// The repulsion pass: `walkGrid(grid, gridRepulse)`.
    ///
    /// Two nested loops over the node pair, exactly as `gridRepulse` has them: the cell
    /// itself with no distance test, then each of the eight neighbours with
    /// `dist < Cell`. The `rand` borrow is the reference's own `rand()` tie-break
    /// sequence, threaded through so the tie-break draws stay in the reference's order.
    pub(super) fn repel(&self, model: &mut Model, rand: &mut GlibcRand) {
        for (key, nodes) in &self.cells {
            within(nodes, nodes, model, rand);
            for (di, dj) in NEIGHBOURS {
                let neighbour = (key.0 + di, key.1 + dj);
                if let Some(others) = self.cells.get(&neighbour) {
                    culled(nodes, others, model, rand);
                }
            }
        }
    }
}

/// `FLOOR(pos / Cell)`, the reference's `(int)floor(...)` on a `double` (`tlayout.c:369`).
///
/// `f64::floor` then a narrowing cast is exactly the C conversion, including for the
/// negative positions this phase produces: C truncates *after* flooring, so it is not the
/// same as a truncating division.
fn cell_of(position: f64) -> i32 {
    (position / CELL).floor() as i32
}

/// One cell against itself: every ordered pair of distinct members, no distance test.
fn within(nodes: &[u32], same: &[u32], model: &mut Model, rand: &mut GlibcRand) {
    for &p in nodes {
        for &q in same {
            if p != q {
                repel(p, q, model, rand);
            }
        }
    }
}

/// One cell against a neighbour: every ordered pair, but only the pairs closer than one
/// cell, which is what makes the grid an approximation rather than a rewrite of the
/// all-pairs sweep.
fn culled(nodes: &[u32], others: &[u32], model: &mut Model, rand: &mut GlibcRand) {
    for &p in nodes {
        for &q in others {
            if separation(p, q, model) < CELL {
                repel(p, q, model, rand);
            }
        }
    }
}

/// `hypot` of the two position deltas, as `applyRep` computes it (`tlayout.c:219`).
fn separation(p: u32, q: u32, model: &Model) -> f64 {
    let (p, q) = (p as usize, q as usize);
    super::distance(model.x[q] - model.x[p], model.y[q] - model.y[p])
}

/// `doRep` with `useNew` set (`tlayout.c:194-210`): `force = K * K / (dist * dist * dist)`.
///
/// `Mlimit` is `HUGE_VAL` at its default (`globals.c:36`), so the reference's cutoff never
/// fires and there is none here. The `while (!(dist > 0))` redraw is the reference's own
/// tie-break: it keeps drawing `5 - rand() % 10` on both axes until the two positions no
/// longer coincide, and the draws come off the same unseeded stream the reference uses.
fn repel(p: u32, q: u32, model: &mut Model, rand: &mut GlibcRand) {
    let (p, q) = (p as usize, q as usize);
    let (mut dx, mut dy) = (model.x[q] - model.x[p], model.y[q] - model.y[p]);
    let mut dist = super::distance(dx, dy);
    // The reference's `while (!(dist > 0))`, spelled out: it is true for a zero distance
    // *and* for a NaN, which a plain `<= 0.0` would not be, so the NaN case is named.
    while dist <= 0.0 || dist.is_nan() {
        dx = rand.jitter();
        dy = rand.jitter();
        dist = super::distance(dx, dy);
    }
    let force = K * K / (dist * dist * dist);
    model.dx[q] += dx * force;
    model.dy[q] += dy * force;
    model.dx[p] -= dx * force;
    model.dy[p] -= dy * force;
}

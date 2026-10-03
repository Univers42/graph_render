//! The bottom-up aggregate over `workers` ranges: the same per-cell arithmetic as the serial
//! loop, and nothing else.
//!
//! `charge::prepare` builds the arena single-threaded; the aggregate over it is the one part
//! of that prologue whose cells are already independent. A cell's body reads only its
//! children's bodies, and in preorder a subtree is the contiguous run `k..skip`, so two
//! cells in disjoint subtrees share nothing. `Runner::run` cuts `0..cells` into fixed
//! ranges (`exec/partition.rs`), each worker writes only its own range of `bodies`, and the
//! per-cell arithmetic is the one [`Body::of`] the serial loop calls — same function, same
//! order, so a worker count is a schedule and not a second program.
//!
//! **The cells a range boundary cuts.** `partition` divides by output count, not by subtree,
//! so a range can end in the middle of a subtree: cell `k` may need a child at an index
//! another worker is writing this instant. Such a cell is left at its `Default` and
//! [`finish`] computes it after the barrier, when every child is final. There are at most
//! `workers` such cells per boundary and they are the ancestors of the boundary cell, so
//! the serial tail is `O(workers × depth)` cells of work over one `O(cells)` scan.
//!
//! Caveat: the scan that finds them is serial and touches every cell, so the parallel
//! aggregate only wins once the cells outnumber `workers` by a wide margin — measured at
//! 1.14 % of a 1M tick's instructions (`docs/measurements/perf-bh-1m.md`), which is inside
//! the run-to-run noise of the tick it is meant to shorten.

use std::ops::Range;

use super::Body;
use super::threshold::{centre, opening_threshold};
use crate::exec::{StepRange, range_at};
use crate::layout::force::quadtree::{Cell, Quadtree};

/// One tick's aggregate: the arena it reads and the constants every cell resolves against.
pub(super) struct Pass<'a> {
    /// The arena, in preorder: what a runner's `partition` divides.
    pub(super) cells: &'a [Cell],
    order: &'a [u32],
    x: &'a [f64],
    y: &'a [f64],
    theta: f64,
    theta2: f64,
}

impl<'a> Pass<'a> {
    pub(super) fn of(tree: &'a Quadtree, (x, y): (&'a [f64], &'a [f64]), theta: f64) -> Self {
        Self {
            cells: tree.cells(),
            order: tree.order(),
            x,
            y,
            theta,
            theta2: theta * theta,
        }
    }

    /// Cell `k`'s body, reading its children at `base`-rebased indices of `bodies`.
    ///
    /// The leaf's centre is its chain head's position, an internal cell's is its children's
    /// mass-weighted centre.
    fn body(&self, bodies: &[Body], k: u32, base: u32) -> Body {
        let cell = self.cells[k as usize];
        let com = if cell.skip == k + 1 {
            let head = self.order[cell.start as usize] as usize;
            (self.x[head], self.y[head])
        } else {
            self.centre_of(bodies, k, base)
        };
        Body::of(com, self.open(&cell), &cell)
    }

    /// The children's mass-weighted centre, slot order, over a span rebased at `base`.
    ///
    /// The same three accumulations, in the same order, over the same values as
    /// [`centre`](super::threshold::centre) — which indexes the column by arena position and
    /// so cannot walk a range's span, whose index `i` is arena cell `base + i`. Where the
    /// span *is* the column (`base == 0`: `finish`'s repair and the one-worker run) the
    /// reference walks it, so the two spellings are held equal by the tests rather than by
    /// one being a copy of the other. Where it is not, the chain comes from `cells`, which
    /// is absolute, and only the bodies are rebased.
    fn centre_of(&self, bodies: &[Body], k: u32, base: u32) -> (f64, f64) {
        let stop = self.cells[k as usize].skip;
        if base == 0 {
            return centre(bodies, k + 1, stop);
        }
        let (mut m, mut cx, mut cy) = (0.0, 0.0, 0.0);
        let mut c = k + 1;
        while c < stop {
            let child = &bodies[(c - base) as usize];
            let cm = f64::from(child.count);
            m += cm;
            cx += cm * child.comx;
            cy += cm * child.comy;
            c = self.cells[c as usize].skip;
        }
        (cx / m, cy / m)
    }

    /// The cell's own opening threshold, `w² / θ²` off the square's width.
    fn open(&self, cell: &Cell) -> f64 {
        opening_threshold(cell.bounds.x1 - cell.bounds.x0, self.theta, self.theta2)
    }

    /// Whether `k`'s subtree leaves the range it was cut into, so its body is not this
    /// range's to compute.
    fn cut(&self, k: u32, range: &Range<u32>) -> bool {
        self.cells[k as usize].skip > range.end
    }
}

impl StepRange for Pass<'_> {
    type Out = Body;

    fn len(&self) -> u32 {
        self.cells.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [Body]) {
        for k in (range.start..range.end).rev() {
            if self.cut(k, &range) {
                continue;
            }
            out[(k - range.start) as usize] = self.body(out, k, range.start);
        }
    }
}

/// The serial reference: every cell, in reverse preorder, as one range's worth of work.
/// The tests' reference arm; the shipped path reaches the same bodies through `run`.
#[cfg(test)]
pub(super) fn serial(pass: &Pass<'_>, bodies: &mut [Body]) {
    for k in (0..pass.len()).rev() {
        bodies[k as usize] = pass.body(bodies, k, 0);
    }
}

/// The tail every range leaves: the cells a boundary cut, computed now that every child is
/// final. `workers` is the count `run` was given, since the plan is a pure function of it.
///
/// **Descending, ranges and cells both.** A cut cell's children sit at higher indices, some
/// of them in the next range — which `run` has finished, but whose *cut* cells this loop has
/// not. Walking down from the last cell therefore reaches a cell only once every cell above
/// it is final, whichever range wrote it.
pub(super) fn finish(pass: &Pass<'_>, bodies: &mut [Body], workers: u32) {
    let (n, workers) = (bodies.len() as u32, workers.max(1));
    for i in (0..n.min(workers)).rev() {
        let range = range_at(n, workers, i);
        for k in (range.start..range.end).rev() {
            if pass.cut(k, &range) {
                bodies[k as usize] = pass.body(bodies, k, 0);
            }
        }
    }
}

//! The collide gather over one contiguous window per cell.
//!
//! A query reads up to nine slot runs, about 25 candidates for about 12 overlaps
//! (`docs/measurements/perf-p3-collide.md`). Filtered in place, every run costs its loop
//! exit, mispredicted, on every query (`docs/measurements/perf-p3-gather.md`). Every slot
//! of a cell reads the same runs, so the cell's candidates are copied once, in `Reads`
//! order, into one window, and each query filters the window in one loop and resolves its
//! hits in a second. The order of the terms in each slot's sum is the runs' order, as before.
//!
//! The window is split by axis (`wx`, `wy`) and the filter reads two candidates per
//! iteration, so the two SSE lanes of one subtract hold candidate `j`'s and `j + 1`'s x
//! rather than one candidate's x and y. The split and the pairing change no term and no
//! order, only how many instructions carry them (`docs/measurements/perf-pm-collide-soa.md`).

use super::{Contact, Grid, Reads, push};
use crate::exec::StepRange;
use std::ops::Range;

/// The window, in candidates: about 5.5 KB of stack per range. A cell with more candidates
/// is read a window at a time, every window refilled on every query.
/// Caveat: in such a crowd every query copies the whole crowd again, so a dense overlap pays a
/// copy per candidate on top of the distance test; 256 is ten times the measured mean, not tuned.
pub(super) const WINDOW: usize = 256;

/// The per-slot gather: reads the built grid only, writes slot `k`'s own delta.
pub(super) struct Gather<'a> {
    pub(super) grid: &'a Grid,
    pub(super) contact: Contact,
}

impl StepRange for Gather<'_> {
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.grid.order.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        let mut window = Window::new();
        for (slot, k) in out.iter_mut().zip(range) {
            *slot = window.delta(self.grid, (k, self.contact));
        }
    }
}

/// Candidates `from..from + len` of one cell's sequence, the slots its runs hold in order.
struct Window {
    reads: Reads,
    /// The sequence's length. Every cell's sequence holds the slot that queries it, so 0
    /// means no cell yet.
    total: usize,
    /// The run holding the cell's own bucket: its first slot, and its place in the sequence.
    own: (u32, usize),
    from: usize,
    len: usize,
    /// The window's places by axis, `wx[j]` and `wy[j]` being candidate `j`. Split so the
    /// filter's two lanes are two candidates, not one candidate's two coordinates.
    wx: [f64; WINDOW],
    wy: [f64; WINDOW],
    slot: [u32; WINDOW],
    hits: [u16; WINDOW],
}

impl Window {
    fn new() -> Window {
        Window {
            reads: Reads {
                cell: (0, 0),
                runs: [(0, 0); 9],
                len: 0,
            },
            total: 0,
            own: (0, 0),
            from: usize::MAX,
            len: 0,
            wx: [0.0; WINDOW],
            wy: [0.0; WINDOW],
            slot: [0; WINDOW],
            hits: [0; WINDOW],
        }
    }

    /// Slot `k`'s half of every overlap it has, in sequence order.
    fn delta(&mut self, grid: &Grid, (k, contact): (u32, Contact)) -> (f64, f64) {
        let [px, py] = grid.at[k as usize];
        let cell = grid.hash.cell_of((px, py));
        if self.total == 0 || self.reads.cell != cell {
            self.point(grid.reads(cell), k);
        }
        let mine = self.own.1 + (k - self.own.0) as usize;
        let mut out = (0.0, 0.0);
        for from in (0..self.total).step_by(WINDOW) {
            if self.from != from {
                self.fill(grid, from);
            }
            let found = self.overlaps((px, py), mine.wrapping_sub(from), (k, contact.d2));
            let (wx, wy, slot) = (&self.wx, &self.wy, &self.slot);
            for &j in &self.hits[..found] {
                let j = j as usize;
                let (dx, dy) = (px - wx[j], py - wy[j]);
                // The jiggle keys, and so the two loads behind them, are read only if this
                // hit needs a jiggle: `push::hit` calls the closure from those two branches.
                let ids = || (grid.order[k as usize], grid.order[slot[j] as usize]);
                let (fx, fy) = push::hit(contact, ids, (dx, dy));
                out.0 += fx;
                out.1 += fy;
            }
        }
        out
    }

    /// Takes `cell`'s runs; `k` is a slot of the cell, so of its own bucket.
    fn point(&mut self, reads: Reads, k: u32) {
        let mut total = 0;
        for &(lo, hi) in &reads.runs[..reads.len] {
            if (lo..hi).contains(&k) {
                self.own = (lo, total);
            }
            total += (hi - lo) as usize;
        }
        (self.reads, self.total, self.from) = (reads, total, usize::MAX);
    }

    fn fill(&mut self, grid: &Grid, from: usize) {
        let (mut skip, mut len) = (from, 0);
        for &(lo, hi) in &self.reads.runs[..self.reads.len] {
            let ahead = skip.min((hi - lo) as usize);
            skip -= ahead;
            let first = lo + ahead as u32;
            for q in first..hi.min(first + (WINDOW - len) as u32) {
                let [x, y] = grid.at[q as usize];
                (self.wx[len], self.wy[len], self.slot[len]) = (x, y, q);
                len += 1;
            }
        }
        (self.from, self.len) = (from, len);
    }

    /// The window's places `resolve` would not skip, ascending, the querying slot `k` at
    /// place `mine` left out. No branch per candidate: about half overlap, so a branch on
    /// the distance test mispredicts. Two candidates per iteration, each lane's `l` with the
    /// one expression `resolve` tests, then both stores in hit order. `k`'s own place reads
    /// NaN in `wx` while the loop runs, and `l < d2` is false for a NaN `l`, which is
    /// `resolve`'s own test negated.
    fn overlaps(&mut self, (px, py): (f64, f64), mine: usize, (k, d2): (u32, f64)) -> usize {
        let held = (mine < self.len).then(|| std::mem::replace(&mut self.wx[mine], f64::NAN));
        debug_assert!(
            held.is_none() || self.slot[mine] == k,
            "slot {k} is not at its place"
        );
        let len = self.len;
        let (wx, wy, hits) = (&self.wx[..len], &self.wy[..len], &mut self.hits);
        let mut found = 0;
        let mut j = 0;
        while j + 1 < len {
            let (dx0, dy0) = (px - wx[j], py - wy[j]);
            let (dx1, dy1) = (px - wx[j + 1], py - wy[j + 1]);
            let c0 = usize::from(dx0 * dx0 + dy0 * dy0 < d2);
            let c1 = usize::from(dx1 * dx1 + dy1 * dy1 < d2);
            // `found <= j + 1 < WINDOW`: the mask changes no index, it drops the bounds check.
            hits[found % WINDOW] = j as u16;
            found += c0;
            hits[found % WINDOW] = (j + 1) as u16;
            found += c1;
            j += 2;
        }
        if j < len {
            let (dx, dy) = (px - wx[j], py - wy[j]);
            hits[found % WINDOW] = j as u16;
            found += usize::from(dx * dx + dy * dy < d2);
        }
        if let Some(x) = held {
            self.wx[mine] = x;
        }
        found
    }
}

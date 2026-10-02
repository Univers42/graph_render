//! Collide over a hashed cell list instead of a quadtree: cells one collide diameter wide,
//! so every overlap a node can have lies in its own cell or one of the eight around it.
//!
//! The overlap correction is Barnes-Hut's own (`barnes_hut/collide.rs::resolve`): the same
//! subtraction read from either end, the same jiggle keys, the same half push. What
//! changes is how the candidates are found. A counting sort puts the nodes in bucket order
//! once per tick, `O(n + buckets)`, and a query reads its three cell rows, three buckets
//! each, against the quadtree's build and walk.
//!
//! Rows are hashed and the cells along a row are not: cell `(cx, cy)` is bucket
//! `row(cy) + cx` modulo `2n` rounded up to a power of two. The grid costs the same however
//! far apart the nodes are, the three cells a query reads in one row are three consecutive
//! buckets, so three runs of the sorted positions, and the next node along the row reads
//! the same memory again. Two cells that share a bucket only add candidates, which the
//! distance test rejects; a bucket two of a query's rows share is read once.
//!
//! Caveat: a bucket shared by two crowded cells makes both cells' queries read both
//! crowds. Rows are spread by the multiplicative hash, not bounded, so the worst case is a
//! quadratic scan of one bucket; a dense overlap (every node within one diameter) is
//! quadratic for Barnes-Hut as well.

use super::frame;
use crate::exec::{Runner, StepRange};
use crate::layout::force::barnes_hut::sim::{How, Sim};
use crate::layout::force::barnes_hut::{Split, step};
use crate::rng::jiggle;
use std::ops::Range;

const PASS_X: u32 = 4;
const PASS_Y: u32 = 5;

/// The cell list over one tick's projected positions.
pub(in crate::layout::force) struct Grid {
    /// The node at each sorted slot. Every node has one, a non-finite one too, so the
    /// charge deposit can walk the nodes in this order.
    pub(super) order: Vec<u32>,
    /// Bucket `b`'s slots are `start[b]..start[b + 1]`.
    start: Vec<u32>,
    /// Each node's bucket, scratch for the sort.
    bucket: Vec<u32>,
    /// The positions in sorted order.
    at: Vec<[f64; 2]>,
    /// `64 - log2(buckets)`: the row hash's top bits are the row's first bucket.
    shift: u32,
    /// `buckets - 1`.
    mask: u64,
    origin: (f64, f64),
    size: f64,
}

/// What every overlap test reads: the squared diameter, the diameter, the jiggle's seed and
/// tick.
#[derive(Clone, Copy)]
struct Contact {
    d2: f64,
    reach: f64,
    seed: u32,
    tick: u32,
}

impl Grid {
    pub(super) fn new(n: u32) -> Grid {
        // At least four, so a row's three buckets are three distinct ones.
        let buckets = (2 * n as usize).next_power_of_two().max(4);
        Grid {
            order: (0..n).collect(),
            start: vec![0; buckets + 1],
            bucket: vec![0; n as usize],
            at: vec![[0.0; 2]; n as usize],
            shift: 64 - buckets.trailing_zeros(),
            mask: buckets as u64 - 1,
            origin: (0.0, 0.0),
            size: 1.0,
        }
    }

    /// Sorts every node into its bucket, stably: inside a bucket, by node index.
    pub(super) fn build(&mut self, (x, y): (&[f64], &[f64]), size: f64) {
        self.size = size;
        self.origin = frame::bounds(x, y).map_or((0.0, 0.0), |(lo, _)| lo);
        self.start.fill(0);
        for i in 0..x.len() {
            let b = self.bucket_of(self.cell_of((x[i], y[i])));
            self.bucket[i] = b;
            self.start[b as usize + 1] += 1;
        }
        for b in 1..self.start.len() {
            self.start[b] += self.start[b - 1];
        }
        for i in 0..x.len() {
            let slot = &mut self.start[self.bucket[i] as usize];
            let k = *slot as usize;
            *slot += 1;
            (self.order[k], self.at[k]) = (i as u32, [x[i], y[i]]);
        }
        // Each `start[b]` now holds bucket `b`'s end, which is bucket `b + 1`'s start.
        let buckets = self.start.len() - 1;
        self.start.copy_within(0..buckets, 1);
        self.start[0] = 0;
    }

    /// The cell of a position. Saturating: a NaN lands in cell 0 and an infinity at the
    /// end of the range, and neither passes the distance test.
    fn cell_of(&self, (x, y): (f64, f64)) -> (i64, i64) {
        let along = |v: f64, o: f64| ((v - o) / self.size) as i64;
        (along(x, self.origin.0), along(y, self.origin.1))
    }

    fn bucket_of(&self, (cx, cy): (i64, i64)) -> u32 {
        (self.row(cy).wrapping_add(cx as u64) & self.mask) as u32
    }

    /// Row `cy`'s bucket for cell `0`, the hash's top bits.
    fn row(&self, cy: i64) -> u64 {
        (cy as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> self.shift
    }

    /// The slot runs a query from `cell` reads: rows `cy - 1..=cy + 1`, buckets left to
    /// right within a row. Consecutive buckets are consecutive slots, so a row's three
    /// buckets are usually one run.
    fn reads(&self, (cx, cy): (i64, i64)) -> Reads {
        let mut reads = Reads {
            cell: (cx, cy),
            runs: [(0, 0); 9],
            len: 0,
        };
        let mut firsts = [0; 3];
        for (r, dy) in (-1..=1).enumerate() {
            let first = self.bucket_of((cx.wrapping_sub(1), cy.wrapping_add(dy))) as u64;
            for b in (first..first + 3).map(|b| b & self.mask) {
                // An earlier row's three buckets `e..e + 3` may hold `b`: read it once.
                if !firsts[..r]
                    .iter()
                    .any(|&e| b.wrapping_sub(e) & self.mask < 3)
                {
                    reads.push(self.start[b as usize], self.start[b as usize + 1]);
                }
            }
            firsts[r] = first;
        }
        reads
    }

    /// Slot `k`'s half of every overlap it has, slots in `reads` order.
    fn delta(&self, k: usize, reads: &Reads, contact: Contact) -> (f64, f64) {
        let [px, py] = self.at[k];
        let mut out = (0.0, 0.0);
        for &(lo, hi) in &reads.runs[..reads.len] {
            let lo = lo as usize;
            for (q, &[qx, qy]) in (lo..).zip(&self.at[lo..hi as usize]) {
                if q != k {
                    let ids = || (self.order[k], self.order[q]);
                    resolve(contact, ids, (px - qx, py - qy), &mut out);
                }
            }
        }
        out
    }
}

/// The slot runs one cell's query reads, at most one per bucket.
struct Reads {
    cell: (i64, i64),
    runs: [(u32, u32); 9],
    len: usize,
}

impl Reads {
    /// Appends slots `lo..hi`, extending the last run when it ends at `lo`: the buckets
    /// between are then empty, so the run gains no other slot.
    fn push(&mut self, lo: u32, hi: u32) {
        if lo == hi {
            return;
        }
        match self.runs[..self.len].last_mut() {
            Some(last) if last.1 == lo => last.1 = hi,
            _ => {
                self.runs[self.len] = (lo, hi);
                self.len += 1;
            }
        }
    }
}

/// Barnes-Hut's overlap correction for one pair, `offset` being the querying node's
/// position minus the other's. A NaN offset is no overlap. `ids` is called only for a
/// jiggle, which most overlaps never need.
fn resolve(
    c: Contact,
    ids: impl Fn() -> (u32, u32),
    (mut dx, mut dy): (f64, f64),
    out: &mut (f64, f64),
) {
    let mut l = dx * dx + dy * dy;
    if l.is_nan() || l >= c.d2 {
        return;
    }
    if dx == 0.0 {
        dx = jiggle(c.seed, c.tick, PASS_X, ids());
        l += dx * dx;
    }
    if dy == 0.0 {
        dy = jiggle(c.seed, c.tick, PASS_Y, ids());
        l += dy * dy;
    }
    let dist = libm::sqrt(l);
    let push = (c.reach - dist) / dist * 0.5;
    out.0 += dx * push;
    out.1 += dy * push;
}

/// The per-slot gather: reads the built grid only, writes slot `k`'s own delta.
struct Gather<'a> {
    grid: &'a Grid,
    contact: Contact,
}

impl StepRange for Gather<'_> {
    type Out = (f64, f64);

    fn len(&self) -> u32 {
        self.grid.order.len() as u32
    }

    /// Consecutive slots mostly share a cell, so a cell's runs are built once per stretch.
    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        let grid = self.grid;
        let mut reads: Option<Reads> = None;
        for (slot, k) in out.iter_mut().zip(range) {
            let cell = grid.cell_of((grid.at[k as usize][0], grid.at[k as usize][1]));
            let reads = match reads {
                Some(ref r) if r.cell == cell => r,
                _ => reads.insert(grid.reads(cell)),
            };
            *slot = grid.delta(k as usize, reads, self.contact);
        }
    }
}

/// The collide pass: project, sort, gather, merge back onto each node.
pub(super) fn apply<R: Runner>(sim: &mut Sim, grid: &mut Grid, how: &mut How<'_, R>) {
    let diameter = 2.0 * sim.params.collide_radius;
    let d2 = diameter * diameter;
    if d2 == 0.0 {
        return;
    }
    for i in 0..sim.x.len() {
        sim.px[i] = sim.x[i] + sim.vx[i];
        sim.py[i] = sim.y[i] + sim.vy[i];
    }
    let contact = Contact {
        d2,
        reach: libm::sqrt(d2),
        seed: sim.seed,
        tick: sim.tick_no,
    };
    grid.build((&sim.px, &sim.py), contact.reach);
    how.runner
        .run(&Gather { grid, contact }, how.workers, how.deltas);
    let split = how.split.splits(Split::Collide);
    step::merge(
        (&mut sim.vx, &mut sim.vy),
        Some(&grid.order),
        how.deltas,
        split,
    );
}

#[cfg(test)]
mod tests;

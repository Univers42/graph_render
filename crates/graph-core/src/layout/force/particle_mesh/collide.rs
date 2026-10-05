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

use super::frame::{self, Bounds};
use super::motion;
use crate::exec::{Runner, StepRange};
use crate::layout::force::barnes_hut::sim::{How, Sim};
use crate::rng::jiggle;
use gather::Gather;
use hash::{Buckets, Hash};
use std::ops::Range;

mod gather;
mod hash;

const PASS_X: u32 = 4;
const PASS_Y: u32 = 5;

/// The cell list over one tick's projected positions.
pub(in crate::layout::force) struct Grid {
    /// The node at each sorted slot. Every node has one, a non-finite one too, so the
    /// charge deposit can walk the nodes in this order.
    pub(super) order: Vec<u32>,
    /// Bucket `b`'s slots are `start[b]..start[b + 1]`. One entry more than the buckets
    /// need: the sort counts two ahead and scatters one ahead, so it ends shifted into
    /// place with no copy.
    start: Vec<u32>,
    /// Each node's bucket while the sort runs, then its slot: `order[slot[i]] == i`.
    pub(super) slot: Vec<u32>,
    /// The positions in sorted order.
    at: Vec<[f64; 2]>,
    hash: Hash,
    /// The origin's bounds fold, one box per block of nodes.
    blocks: Vec<Bounds>,
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
    /// The bucket count `n` nodes get: a power of two so [`Hash`](struct@Hash) masks instead of
    /// dividing, and at least four so a row's three buckets are three distinct ones.
    fn buckets(n: u32) -> usize {
        (2 * n as usize).next_power_of_two().max(4)
    }

    pub(super) fn new(n: u32) -> Grid {
        let buckets = Self::buckets(n);
        Grid {
            order: (0..n).collect(),
            start: vec![0; buckets + 2],
            slot: (0..n).collect(),
            at: vec![[0.0; 2]; n as usize],
            hash: Hash {
                shift: 64 - buckets.trailing_zeros(),
                mask: buckets as u64 - 1,
                origin: (0.0, 0.0),
                size: 1.0,
            },
            blocks: Vec::with_capacity(n.div_ceil(frame::BLOCK) as usize),
        }
    }

    /// This grid over `n` nodes, in place. [`build`](Self::build) refills `start`, `order`,
    /// `slot` and `at` on every tick, so only their lengths matter — except `order` and
    /// `slot`, which the tick's charge reads *before* that tick's collide rebuilds them (the
    /// charge pass comes first in [`tick`](super::tick)), and [`Hash`](struct@Hash), which is
    /// keyed on the bucket count the node count sets.
    ///
    /// So `order` and `slot` are put back to the identity permutation a fresh grid holds:
    /// a charge read against the last sort's permutation would deposit the new rows into
    /// the cells of whatever nodes the last sort put in their slots.
    pub(super) fn grow(&mut self, n: u32) {
        let buckets = Self::buckets(n);
        self.start.resize(buckets + 2, 0);
        self.hash = Hash {
            shift: 64 - buckets.trailing_zeros(),
            mask: buckets as u64 - 1,
            origin: (0.0, 0.0),
            size: 1.0,
        };
        let n = n as usize;
        self.order.resize(n, 0);
        for (i, node) in self.order.iter_mut().enumerate() {
            *node = i as u32;
        }
        self.slot.resize(n, 0);
        self.slot.clone_from_slice(&self.order);
        self.at.resize(n, [0.0; 2]);
    }

    /// Sorts every node into its bucket, stably: inside a bucket, by node index. The
    /// bounds, the hashing and the sorted positions run through `runner`; the counting
    /// sort is one thread's.
    pub(super) fn build<R: Runner>(
        &mut self,
        xy: (&[f64], &[f64]),
        size: f64,
        (runner, workers): (&R, u32),
    ) {
        let found = frame::bounds(xy, runner, workers, &mut self.blocks);
        let origin = found.map_or((0.0, 0.0), |(lo, _)| lo);
        self.hash = Hash {
            origin,
            size,
            ..self.hash
        };
        let hash = self.hash;
        runner.run(&Buckets { hash, xy }, workers, &mut self.slot);
        {
            let _span = crate::pmprobe::Span::new(0);
            self.start.fill(0);
            for &b in &self.slot {
                self.start[b as usize + 2] += 1;
            }
            for b in 1..self.start.len() {
                self.start[b] += self.start[b - 1];
            }
        }
        // `start[b + 1]` is bucket `b`'s start and its cursor; it ends as bucket `b`'s
        // end, which is bucket `b + 1`'s start.
        {
            let _span = crate::pmprobe::Span::new(1);
            for (i, slot) in self.slot.iter_mut().enumerate() {
                let next = &mut self.start[*slot as usize + 1];
                let k = *next;
                *next += 1;
                self.order[k as usize] = i as u32;
                *slot = k;
            }
        }
        let sorted = Sorted {
            order: &self.order,
            xy,
        };
        runner.run(&sorted, workers, &mut self.at);
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
            let first =
                self.hash
                    .bucket_of((cx.wrapping_sub(1), cy.wrapping_add(dy))) as u64;
            for b in (first..first + 3).map(|b| b & self.hash.mask) {
                // An earlier row's three buckets `e..e + 3` may hold `b`: read it once.
                if !firsts[..r]
                    .iter()
                    .any(|&e| b.wrapping_sub(e) & self.hash.mask < 3)
                {
                    reads.push(self.start[b as usize], self.start[b as usize + 1]);
                }
            }
            firsts[r] = first;
        }
        reads
    }
}

/// The positions in sorted order: slot `k` reads node `order[k]`.
struct Sorted<'a> {
    order: &'a [u32],
    xy: (&'a [f64], &'a [f64]),
}

impl StepRange for Sorted<'_> {
    type Out = [f64; 2];

    fn len(&self) -> u32 {
        self.order.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [[f64; 2]]) {
        let order = &self.order[range.start as usize..range.end as usize];
        for (at, &i) in out.iter_mut().zip(order) {
            *at = [self.xy.0[i as usize], self.xy.1[i as usize]];
        }
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
    let dist = f64::sqrt(l);
    let push = (c.reach - dist) / dist * 0.5;
    out.0 += dx * push;
    out.1 += dy * push;
}

/// The collide pass: project, sort, gather into `how.deltas` in slot order. False when
/// collide is off and no delta was gathered.
pub(super) fn apply<R: Runner>(sim: &mut Sim, grid: &mut Grid, how: &mut How<'_, R>) -> bool {
    let diameter = 2.0 * sim.params.collide_radius;
    let d2 = diameter * diameter;
    if d2 == 0.0 {
        return false;
    }
    let on = (how.runner, how.workers);
    motion::project(sim, on);
    let contact = Contact {
        d2,
        reach: f64::sqrt(d2),
        seed: sim.seed,
        tick: sim.tick_no,
    };
    grid.build((&sim.px, &sim.py), contact.reach, on);
    how.runner
        .run(&Gather { grid, contact }, how.workers, how.deltas);
    true
}

#[cfg(test)]
mod tests;

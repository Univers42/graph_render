//! Collide over a hashed cell list instead of a quadtree: cells one collide diameter wide,
//! so every overlap a node can have lies in its own cell or one of the eight around it.
//!
//! The overlap correction is Barnes-Hut's own (`barnes_hut/collide.rs::resolve`): the same
//! subtraction read from either end, the same jiggle keys, the same half push. What
//! changes is how the candidates are found. A counting sort puts the nodes in bucket order
//! once per tick, `O(n + buckets)`, and a query reads nine contiguous ranges of that order,
//! against the quadtree's build and walk.
//!
//! Cells are hashed into `2n` rounded up to a power of two buckets, so the grid costs the
//! same however far apart the nodes are. Two cells that share a bucket only add
//! candidates, which the distance test rejects; a bucket two of the nine neighbour cells
//! share is read once.
//!
//! Caveat: a bucket shared by two crowded cells makes both cells' queries read both
//! crowds. Collisions are spread by the multiplicative hash, not bounded, so the worst case
//! is a quadratic scan of one bucket; a dense overlap (every node within one diameter) is
//! quadratic for Barnes-Hut as well.

use super::frame;
use crate::exec::{Runner, StepRange};
use crate::layout::force::barnes_hut::sim::{How, Sim};
use crate::layout::force::barnes_hut::{Split, step};
use crate::rng::jiggle;
use std::ops::Range;

const PASS_X: u32 = 4;
const PASS_Y: u32 = 5;

/// The nine cells a query reads, its own first.
const NEIGHBOURS: [(i64, i64); 9] = [
    (0, 0),
    (-1, -1),
    (0, -1),
    (1, -1),
    (-1, 0),
    (1, 0),
    (-1, 1),
    (0, 1),
    (1, 1),
];

/// The cell list over one tick's projected positions.
pub(in crate::layout::force) struct Grid {
    /// The node at each sorted slot. Every node has one, a non-finite one too, so the
    /// charge deposit can walk the nodes in this order.
    pub(super) order: Vec<u32>,
    /// Bucket `b`'s slots are `start[b]..start[b + 1]`.
    start: Vec<u32>,
    /// Each node's bucket, scratch for the sort.
    bucket: Vec<u32>,
    sx: Vec<f64>,
    sy: Vec<f64>,
    /// `64 - log2(buckets)`: the hash's top bits are the bucket.
    shift: u32,
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
        let buckets = (2 * n as usize).next_power_of_two().max(2);
        Grid {
            order: (0..n).collect(),
            start: vec![0; buckets + 1],
            bucket: vec![0; n as usize],
            sx: vec![0.0; n as usize],
            sy: vec![0.0; n as usize],
            shift: 64 - buckets.trailing_zeros(),
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
            (self.order[k], self.sx[k], self.sy[k]) = (i as u32, x[i], y[i]);
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
        let mixed = (cx as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ cy as u64;
        (mixed.wrapping_mul(0xC2B2_AE3D_27D4_EB4F) >> self.shift) as u32
    }

    /// Slot `k`'s half of every overlap it has, neighbour buckets in [`NEIGHBOURS`] order
    /// and slots in sorted order within each.
    fn delta(&self, k: usize, contact: Contact) -> (f64, f64) {
        let (cx, cy) = self.cell_of((self.sx[k], self.sy[k]));
        let mut seen = [u32::MAX; 9];
        let mut out = (0.0, 0.0);
        for (slot, &(dx, dy)) in NEIGHBOURS.iter().enumerate() {
            let b = self.bucket_of((cx.wrapping_add(dx), cy.wrapping_add(dy)));
            if seen[..slot].contains(&b) {
                continue;
            }
            seen[slot] = b;
            for q in self.start[b as usize] as usize..self.start[b as usize + 1] as usize {
                if q != k {
                    let offset = (self.sx[k] - self.sx[q], self.sy[k] - self.sy[q]);
                    resolve(contact, (self.order[k], self.order[q]), offset, &mut out);
                }
            }
        }
        out
    }
}

/// Barnes-Hut's overlap correction for one pair, `offset` being the querying node's
/// position minus the other's. A NaN offset is no overlap.
fn resolve(c: Contact, ids: (u32, u32), (mut dx, mut dy): (f64, f64), out: &mut (f64, f64)) {
    let mut l = dx * dx + dy * dy;
    if l.is_nan() || l >= c.d2 {
        return;
    }
    if dx == 0.0 {
        dx = jiggle(c.seed, c.tick, PASS_X, ids);
        l += dx * dx;
    }
    if dy == 0.0 {
        dy = jiggle(c.seed, c.tick, PASS_Y, ids);
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

    fn step_range(&self, range: Range<u32>, out: &mut [(f64, f64)]) {
        for (slot, k) in out.iter_mut().zip(range) {
            *slot = self.grid.delta(k as usize, self.contact);
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
    how.runner.run(&Gather { grid, contact }, how.workers, how.deltas);
    let split = how.split.splits(Split::Collide);
    step::merge((&mut sim.vx, &mut sim.vy), Some(&grid.order), how.deltas, split);
}

#[cfg(test)]
mod tests;

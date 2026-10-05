//! The collide grid's counting sort, walked in the *previous* tick's order.
//!
//! [`Grid::build`](super::Grid::build) sorts the nodes into their buckets once per tick,
//! one thread, and the sort is the largest serial piece of a particle-mesh tick
//! (`docs/measurements/perf-pm-coherent-sort.md`). It walks the nodes in dense index
//! order, but the buckets are in *spatial* order, so every count and every scatter is a
//! random access into the 8 MB `start` table at 1M nodes.
//!
//! Nodes move little from one tick to the next. So this module keeps the last tick's
//! `order` and walks *it*: a pass computes `bk[k]`, the bucket of the node at slot `k`, and
//! the count and the scatter then walk slots. Neighbouring slots hold neighbouring nodes,
//! so the same counts land in nearly the same buckets one after another and the accesses
//! are nearly sequential. The bytes are unchanged: each bucket gets the same set of nodes,
//! and the per-bucket sort below restores the ascending node index that today's node-order
//! scatter gives for free.
//!
//! The first build after [`Grid::new`](super::Grid::new) or
//! [`Grid::grow`](super::Grid::grow) starts from the identity, which is exactly today's
//! algorithm — the coherence buys nothing there and costs nothing.
//!
//! Caveat: the per-bucket sort is a small sort per bucket, so a layout whose nodes all
//! crowd one bucket pays a sort over the whole node set where today's scatter paid a
//! sequential pass. Insertion is used only up to [`INSERTION`] entries; above it the run
//! is handed to `sort_unstable`, and the keys (`(node, slot)` packed) are unique, so
//! stability is moot either way.

use super::Grid;
use super::hash::Hash;
use crate::exec::{Runner, StepRange};
use std::ops::Range;

/// A bucket of at most this many slots is sorted by insertion; a longer one by
/// `sort_unstable`.
const INSERTION: usize = 32;

/// The sort's own buffers, sized in [`Grid::new`](super::Grid::new) and
/// [`Grid::grow`](super::Grid::grow) with the rest.
pub(super) struct Scratch {
    /// The bucket of each slot: the previous order's node at each slot, hashed again.
    buckets: Vec<u32>,
    /// The scatter's `(node, slot)` pairs, one per slot, at the new slot.
    packed: Vec<u64>,
    /// The new slot of each old slot, over `buckets`' buffer once it is spent.
    spare_slot: Vec<u32>,
}

impl Scratch {
    pub(super) fn new(n: u32) -> Scratch {
        Scratch {
            buckets: vec![0; n as usize],
            packed: vec![0; n as usize],
            spare_slot: vec![0; n as usize],
        }
    }

    /// The buffers only ever need room for `n` slots: `buckets` is refilled by the pass,
    /// `packed` by every scatter and `spare_slot` by the invert, each over all `n` slots.
    pub(super) fn grow(&mut self, n: u32) {
        let n = n as usize;
        self.buckets.resize(n, 0);
        self.packed.resize(n, 0);
        self.spare_slot.resize(n, 0);
    }
}

/// Each slot's bucket: `bk[k] = bucket(cell(xy[order[k]]))`, gathered in the previous
/// order rather than walked in node order.
struct Buckets<'a> {
    hash: Hash,
    order: &'a [u32],
    xy: (&'a [f64], &'a [f64]),
}

impl StepRange for Buckets<'_> {
    type Out = u32;

    fn len(&self) -> u32 {
        self.order.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
        let slots = range.start as usize..range.end as usize;
        let xy = self.xy;
        let hash = self.hash;
        for (bucket, &i) in out.iter_mut().zip(&self.order[slots]) {
            let i = i as usize;
            *bucket = hash.bucket_of(hash.cell_of((xy.0[i], xy.1[i])));
        }
    }
}

/// `slot[i] = moved[old slot of i]`: the new permutation's inverse, gathered by node from
/// the previous tick's `slot`. The gather is a node-ordered pass so the workers share it.
struct Invert<'a> {
    slot: &'a [u32],
    moved: &'a [u32],
}

impl StepRange for Invert<'_> {
    type Out = u32;

    fn len(&self) -> u32 {
        self.slot.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
        let nodes = range.start as usize..range.end as usize;
        for (new, &i) in out.iter_mut().zip(&self.slot[nodes]) {
            *new = self.moved[i as usize];
        }
    }
}

impl Grid {
    /// The counting sort, in the previous tick's order. The `runner` passes are the
    /// per-slot buckets and the new `slot`; the count, the prefix, the scatter, the
    /// per-bucket order and the unpack are one thread's, as they are today.
    pub(super) fn sort<R: Runner>(&mut self, xy: (&[f64], &[f64]), (runner, workers): (&R, u32)) {
        let buckets = Buckets {
            hash: self.hash,
            order: &self.order,
            xy,
        };
        runner.run(&buckets, workers, &mut self.scratch.buckets);
        self.count();
        self.scatter();
        self.unpack();
        let invert = Invert {
            slot: &self.slot,
            moved: &self.scratch.buckets,
        };
        runner.run(&invert, workers, &mut self.scratch.spare_slot);
        std::mem::swap(&mut self.slot, &mut self.scratch.spare_slot);
    }

    /// `start` from zero to the bucket `b`'s end, counting one bucket ahead so the
    /// scatter can shift the table into place with no copy.
    fn count(&mut self) {
        self.start.fill(0);
        for &b in &self.scratch.buckets {
            self.start[b as usize + 2] += 1;
        }
        for b in 1..self.start.len() {
            self.start[b] += self.start[b - 1];
        }
    }

    /// Each slot's node into its bucket's run, carrying the slot it came from so the
    /// unpack can invert the move. Then every bucket's run ascending by node index.
    fn scatter(&mut self) {
        let order = &self.order;
        for (k, &b) in self.scratch.buckets.iter().enumerate() {
            let next = &mut self.start[b as usize + 1];
            let c = *next;
            *next += 1;
            self.scratch.packed[c as usize] = (order[k] as u64) << 32 | k as u64;
        }
        self.restore_order();
    }

    /// Every bucket's run ascending by node index, which is the contract `build` states.
    /// A run already ascending is left alone, which is the common case when nothing moved
    /// between its nodes.
    fn restore_order(&mut self) {
        let end = self.start.len() - 1;
        for b in 0..end {
            let run = self.start[b] as usize..self.start[b + 1] as usize;
            sort_run(&mut self.scratch.packed[run]);
        }
    }

    /// `order` from the sorted pairs, and each old slot's new slot over `buckets`, which
    /// the scatter has read to the end.
    fn unpack(&mut self) {
        let buckets = &mut self.scratch.buckets;
        for (c, &p) in self.scratch.packed.iter().enumerate() {
            self.order[c] = (p >> 32) as u32;
            buckets[(p & 0xFFFF_FFFF) as usize] = c as u32;
        }
    }
}

/// One bucket's run ascending. The keys are unique, so stability buys nothing.
fn sort_run(run: &mut [u64]) {
    if run.windows(2).all(|w| w[0] < w[1]) {
        return;
    }
    if run.len() <= INSERTION {
        for i in 1..run.len() {
            let key = run[i];
            let mut j = i;
            while j > 0 && run[j - 1] > key {
                run[j] = run[j - 1];
                j -= 1;
            }
            run[j] = key;
        }
        return;
    }
    run.sort_unstable();
}

#[cfg(test)]
mod tests;

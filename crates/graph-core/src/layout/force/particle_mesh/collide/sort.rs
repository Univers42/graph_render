//! The grid's sort: every node into bucket order, stably, so a bucket's slots hold its nodes by
//! ascending index. The result is a pure function of the keys `(bucket, node)`, so the one-thread
//! counting sort and the threaded merge sort write the same four columns.
//!
//! Threaded, every pass a gather (`prompt.md` §6 D10): each range sorts its own keys, read in the
//! previous build's slot order, so they arrive nearly sorted; each output range finds where it
//! starts in every run by bisecting the key space, then merges the runs' heads; the positions,
//! the bucket starts and the slots are read off the merged order.

use super::Grid;
use super::hash::{Buckets, Hash};
use crate::exec::{Runner, StepRange, ranges};
use std::ops::Range;

/// The most runs the merge reads: its cursors live on the stack.
const MAX_RUNS: u32 = 64;

impl Grid {
    /// Sorts the nodes at `xy` into `order`, `at`, `start` and `slot`.
    pub(super) fn sort<R: Runner>(&mut self, xy: (&[f64], &[f64]), (runner, workers): (&R, u32)) {
        if workers < 2 {
            self.count(xy, runner);
        } else {
            self.merge(xy, (runner, workers.min(MAX_RUNS)));
        }
    }

    /// One thread's counting sort: histogram, prefix sum, scatter, `O(n + buckets)`.
    fn count<R: Runner>(&mut self, xy: (&[f64], &[f64]), runner: &R) {
        runner.run(
            &Buckets {
                hash: self.hash,
                xy,
            },
            1,
            &mut self.slot,
        );
        self.start.fill(0);
        for &b in &self.slot {
            self.start[b as usize + 1] += 1;
        }
        for b in 1..self.start.len() {
            self.start[b] += self.start[b - 1];
        }
        let (x, y) = xy;
        for (i, slot) in self.slot.iter_mut().enumerate() {
            let next = &mut self.start[*slot as usize];
            let k = *next;
            *next += 1;
            (self.order[k as usize], self.at[k as usize]) = (i as u32, [x[i], y[i]]);
            *slot = k;
        }
        // Each `start[b]` now holds bucket `b`'s end, which is bucket `b + 1`'s start.
        let buckets = self.start.len() - 1;
        self.start.copy_within(0..buckets, 1);
        self.start[0] = 0;
    }

    /// Sorted runs, merged, then the columns read off the merged order: five range passes.
    fn merge<R: Runner>(&mut self, xy: (&[f64], &[f64]), (runner, workers): (&R, u32)) {
        let (hash, n) = (self.hash, self.order.len() as u32);
        let runs = Runs {
            hash,
            xy,
            order: &self.order,
        };
        runner.run(&runs, workers, &mut self.runs);
        let runs = &self.runs;
        debug_assert!(
            ranges(n, workers).all(|r| runs[r.start as usize..r.end as usize].is_sorted())
        );
        runner.run(&Merge { runs, workers }, workers, &mut self.order);
        let order = &self.order;
        runner.run(&Place { order, xy }, workers, &mut self.at);
        let len = self.start.len() as u32;
        let at = &self.at;
        runner.run(&Starts { hash, at, len }, workers, &mut self.start);
        let start = &self.start;
        let slots = Slots {
            hash,
            xy,
            order,
            start,
        };
        runner.run(&slots, workers, &mut self.slot);
    }
}

/// Each range's keys `bucket << 32 | node`, in the previous build's slot order, sorted in place.
///
/// Unlike the other kernels its output depends on the division: one sorted run per range.
/// [`Merge`] reads the runs at [`ranges`]`(n, workers)`, which is the plan every runner runs
/// (`Runner::run`'s doc), and a debug build checks each run is sorted.
struct Runs<'a> {
    hash: Hash,
    xy: (&'a [f64], &'a [f64]),
    order: &'a [u32],
}

impl StepRange for Runs<'_> {
    type Out = u64;

    fn len(&self) -> u32 {
        self.order.len() as u32
    }

    /// Unstable is enough: the keys are distinct, so every sort of them is the same.
    fn step_range(&self, range: Range<u32>, out: &mut [u64]) {
        let (x, y) = self.xy;
        let order = &self.order[range.start as usize..range.end as usize];
        for (key, &i) in out.iter_mut().zip(order) {
            let b = self.hash.bucket_at((x[i as usize], y[i as usize]));
            *key = u64::from(b) << 32 | u64::from(i);
        }
        out.sort_unstable();
    }
}

/// The node at each sorted slot: the runs merged.
struct Merge<'a> {
    runs: &'a [u64],
    workers: u32,
}

impl Merge<'_> {
    fn runs(&self) -> impl Iterator<Item = Range<u32>> {
        ranges(self.runs.len() as u32, self.workers)
    }

    /// How many keys are below `x`, over every run.
    fn rank(&self, x: u64) -> u32 {
        let below =
            |r: Range<u32>| self.runs[r.start as usize..r.end as usize].partition_point(|&k| k < x);
        self.runs().map(|r| below(r) as u32).sum()
    }

    /// The `a`-th smallest key: the largest `x` with at most `a` keys below it. At most 64
    /// bisections of the key space, each a binary search per run.
    fn kth(&self, a: u32) -> u64 {
        let (mut lo, mut hi) = (0, u64::MAX);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if self.rank(mid) <= a {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        lo
    }
}

impl StepRange for Merge<'_> {
    type Out = u32;

    fn len(&self) -> u32 {
        self.runs.len() as u32
    }

    /// Takes from the run with the smallest head until its head passes the second smallest,
    /// so a stretch drawn from one run costs one comparison per key. `u64::MAX` marks a
    /// spent run: no key is that large, as a node index is below `u32::MAX`.
    fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
        let first = self.kth(range.start);
        let mut cursor = [(0, 0); MAX_RUNS as usize];
        let mut head = [u64::MAX; MAX_RUNS as usize];
        let mut len = 0;
        for r in self.runs() {
            let keys = &self.runs[r.start as usize..r.end as usize];
            let at = r.start as usize + keys.partition_point(|&k| k < first);
            cursor[len] = (at, r.end as usize);
            head[len] = if at < r.end as usize {
                self.runs[at]
            } else {
                u64::MAX
            };
            len += 1;
        }
        let mut k = 0;
        while k < out.len() {
            let (j, bound) = smallest_two(&head[..len]);
            loop {
                out[k] = head[j] as u32;
                k += 1;
                let (at, end) = &mut cursor[j];
                *at += 1;
                head[j] = if *at < *end { self.runs[*at] } else { u64::MAX };
                if k == out.len() || head[j] > bound {
                    break;
                }
            }
        }
    }
}

/// The index of the smallest head and the value of the second smallest.
fn smallest_two(head: &[u64]) -> (usize, u64) {
    let (mut best, mut second) = (0, u64::MAX);
    for (j, &h) in head.iter().enumerate().skip(1) {
        if h < head[best] {
            (best, second) = (j, head[best]);
        } else if h < second {
            second = h;
        }
    }
    (best, second)
}

/// The positions in sorted order.
struct Place<'a> {
    order: &'a [u32],
    xy: (&'a [f64], &'a [f64]),
}

impl StepRange for Place<'_> {
    type Out = [f64; 2];

    fn len(&self) -> u32 {
        self.order.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [[f64; 2]]) {
        let (x, y) = self.xy;
        let order = &self.order[range.start as usize..range.end as usize];
        for (at, &i) in out.iter_mut().zip(order) {
            *at = [x[i as usize], y[i as usize]];
        }
    }
}

/// `start[b]`, the slots before bucket `b`: one binary search per range over the sorted
/// positions' buckets, then a walk.
struct Starts<'a> {
    hash: Hash,
    at: &'a [[f64; 2]],
    len: u32,
}

impl StepRange for Starts<'_> {
    type Out = u32;

    fn len(&self) -> u32 {
        self.len
    }

    fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
        let bucket = |&[x, y]: &[f64; 2]| self.hash.bucket_at((x, y));
        let mut k = self.at.partition_point(|p| bucket(p) < range.start);
        for (start, b) in out.iter_mut().zip(range) {
            k += self.at[k..].iter().take_while(|p| bucket(p) < b).count();
            *start = k as u32;
        }
    }
}

/// Each node's slot, `order[slot[i]] == i`: a binary search inside its own bucket, whose
/// slots hold its nodes by ascending index.
struct Slots<'a> {
    hash: Hash,
    xy: (&'a [f64], &'a [f64]),
    order: &'a [u32],
    start: &'a [u32],
}

impl StepRange for Slots<'_> {
    type Out = u32;

    fn len(&self) -> u32 {
        self.order.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
        let (x, y) = self.xy;
        for (slot, i) in out.iter_mut().zip(range) {
            let b = self.hash.bucket_at((x[i as usize], y[i as usize])) as usize;
            let (lo, hi) = (self.start[b] as usize, self.start[b + 1] as usize);
            *slot = (lo + self.order[lo..hi].partition_point(|&o| o < i)) as u32;
        }
    }
}

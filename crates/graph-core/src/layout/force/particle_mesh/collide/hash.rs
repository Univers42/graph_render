//! The cell hash, and the pass that hashes every node through the `Runner`.

use crate::exec::StepRange;
use std::ops::Range;

/// Cell `(cx, cy)` is bucket `row(cy) + cx` modulo the bucket count.
#[derive(Clone, Copy)]
pub(super) struct Hash {
    /// `64 - log2(buckets)`: the row hash's top bits are the row's first bucket.
    pub(super) shift: u32,
    /// `buckets - 1`.
    pub(super) mask: u64,
    pub(super) origin: (f64, f64),
    pub(super) size: f64,
}

impl Hash {
    /// The cell of a position. Saturating: a NaN lands in cell 0 and an infinity at the
    /// end of the range, and neither passes the distance test.
    pub(super) fn cell_of(&self, (x, y): (f64, f64)) -> (i64, i64) {
        let along = |v: f64, o: f64| ((v - o) / self.size) as i64;
        (along(x, self.origin.0), along(y, self.origin.1))
    }

    pub(super) fn bucket_at(&self, xy: (f64, f64)) -> u32 {
        self.bucket_of(self.cell_of(xy))
    }

    pub(super) fn bucket_of(&self, (cx, cy): (i64, i64)) -> u32 {
        (self.row(cy).wrapping_add(cx as u64) & self.mask) as u32
    }

    /// Row `cy`'s bucket for cell `0`, the hash's top bits.
    fn row(&self, cy: i64) -> u64 {
        (cy as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> self.shift
    }
}

/// Each node's bucket, one output per node.
pub(super) struct Buckets<'a> {
    pub(super) hash: Hash,
    pub(super) xy: (&'a [f64], &'a [f64]),
}

impl StepRange for Buckets<'_> {
    type Out = u32;

    fn len(&self) -> u32 {
        self.xy.0.len() as u32
    }

    fn step_range(&self, range: Range<u32>, out: &mut [u32]) {
        let nodes = range.start as usize..range.end as usize;
        let xy = self.xy.0[nodes.clone()].iter().zip(&self.xy.1[nodes]);
        for (bucket, (&x, &y)) in out.iter_mut().zip(xy) {
            *bucket = self.hash.bucket_at((x, y));
        }
    }
}

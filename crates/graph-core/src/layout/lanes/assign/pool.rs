//! The free pool: the lanes no vertex holds and no edge is carrying, smallest first, and how
//! many lanes were ever opened. Split out of `assign.rs` so that file stays inside the house
//! limit; the rule it serves is stated there.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// Released lanes, smallest first, and how many lanes were ever opened.
pub(super) struct Pool {
    free: BinaryHeap<Reverse<u32>>,
    width: u32,
    /// Which lanes are in `free`. Debug-only: the one-reservation invariant is a property of
    /// the reservations being indexed by lane, and the check has to be O(1) to run on every
    /// push.
    #[cfg(debug_assertions)]
    free_flag: Vec<bool>,
}

impl Pool {
    pub(super) fn new() -> Self {
        Self {
            free: BinaryHeap::new(),
            width: 0,
            #[cfg(debug_assertions)]
            free_flag: Vec::new(),
        }
    }

    /// The smallest released lane, or a new one when the pool is empty. `width` only ever
    /// grows, so a lane handed back is always one the pool has already seen.
    pub(super) fn take(&mut self) -> u32 {
        if let Some(Reverse(lane)) = self.free.pop() {
            self.mark_taken(lane);
            return lane;
        }
        self.width += 1;
        self.mark_opened(self.width - 1);
        self.width - 1
    }

    pub(super) fn give(&mut self, lane: u32) {
        self.free.push(Reverse(lane));
        self.mark_given(lane);
    }

    #[cfg(debug_assertions)]
    fn mark_taken(&mut self, lane: u32) {
        self.free_flag[lane as usize] = false;
    }

    #[cfg(not(debug_assertions))]
    fn mark_taken(&mut self, _lane: u32) {}

    #[cfg(debug_assertions)]
    fn mark_opened(&mut self, lane: u32) {
        if self.free_flag.len() <= lane as usize {
            self.free_flag.resize(lane as usize + 1, false);
        }
    }

    #[cfg(not(debug_assertions))]
    fn mark_opened(&mut self, _lane: u32) {}

    #[cfg(debug_assertions)]
    fn mark_given(&mut self, lane: u32) {
        self.free_flag[lane as usize] = true;
    }

    #[cfg(not(debug_assertions))]
    fn mark_given(&mut self, _lane: u32) {}

    /// Whether `lane` is currently released, read by the reservations' invariant check.
    #[cfg(debug_assertions)]
    pub(super) fn free_flag(&self, lane: u32) -> bool {
        self.free_flag[lane as usize]
    }
}

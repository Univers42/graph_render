//! The lanes reserved for each vertex, one intrusive list per vertex, and the one-reservation
//! invariant that keeps a vertex off an edge running past it. Split out of `assign.rs` so that
//! file stays inside the house limit; the rule the reservations serve is stated there.

use super::NONE;
use super::pool::Pool;

/// The lanes reserved for each vertex. A lane is in at most one reservation at a time, so
/// `next` is indexed by lane.
pub(super) struct Reserved {
    head: Vec<u32>,
    next: Vec<u32>,
    min: Vec<u32>,
    /// How many reservations hold each lane. Debug-only: 0 or 1, and the whole correctness
    /// argument rests on it never being 2.
    #[cfg(debug_assertions)]
    held: Vec<u32>,
}

impl Reserved {
    pub(super) fn of(n: u32) -> Self {
        Self {
            head: vec![NONE; n as usize],
            next: Vec::new(),
            min: vec![NONE; n as usize],
            #[cfg(debug_assertions)]
            held: Vec::new(),
        }
    }

    /// `v`'s smallest reserved lane, or [`NONE`]. Rule D reads this for every forward edge,
    /// so it is one indexed read and not a walk.
    pub(super) fn smallest(&self, v: u32) -> u32 {
        self.min[v as usize]
    }

    /// `pool` is read only by the debug-only invariant check below, so it is named `_pool`:
    /// in a release build there is no check and the argument is genuinely unused.
    pub(super) fn push(&mut self, v: u32, lane: u32, _pool: &Pool) {
        self.hold(lane);
        #[cfg(debug_assertions)]
        self.assert_sole(lane, v, _pool);
        if self.next.len() <= lane as usize {
            self.next.resize(lane as usize + 1, NONE);
        }
        self.next[lane as usize] = self.head[v as usize];
        self.head[v as usize] = lane;
        self.min[v as usize] = self.min[v as usize].min(lane);
    }

    /// Empties `v`'s reservation and returns its smallest lane, or [`NONE`]. Every other lane
    /// in it goes back to `pool`.
    ///
    /// `keep` is released too, and that is the whole point: it is not given to `pool`, because
    /// the caller takes it as `v`'s own lane, but it is no longer *reserved for `v`* — a
    /// forward edge re-reserves it for its target. A lane the caller never re-reserves is
    /// given to the pool by the caller instead.
    pub(super) fn settle(&mut self, v: u32, pool: &mut Pool) -> u32 {
        let keep = self.min[v as usize];
        let mut lane = self.head[v as usize];
        while lane != NONE {
            let next = self.next[lane as usize];
            #[cfg(debug_assertions)]
            self.assert_sole(lane, v, pool);
            if lane != keep {
                pool.give(lane);
                self.release(lane);
            }
            lane = next;
        }
        self.head[v as usize] = NONE;
        self.min[v as usize] = NONE;
        if keep != NONE {
            self.release(keep);
        }
        keep
    }

    /// The give-side half of the one-reservation invariant
    /// (`docs/decisions/dag-lanes-merge.md` condition 2). `assert_sole` checks a lane being
    /// pushed; rule D adds the only path that hands a lane back to the pool, so the lane going
    /// the other way has to be checked too — a lane in a reservation that is also free is the
    /// vertex-on-an-edge bug, from the other direction.
    #[cfg(debug_assertions)]
    pub(super) fn assert_unheld(&self, lane: u32) {
        // `get`, not an index: a lane nothing ever reserved has no entry in `held`, and
        // "held by nobody" is exactly the state the give-back wants to confirm.
        debug_assert_eq!(
            self.held.get(lane as usize).copied().unwrap_or(0),
            0,
            "lane {lane} is given to the pool while reserved"
        );
    }

    // No `cfg(not(debug_assertions))` twin for `assert_unheld`, unlike `hold`/`release`: its
    // only call site is itself behind `debug_assertions`, so a release stub is dead code.

    /// The push-side invariant in code, not only in the doc above
    /// (`docs/decisions/dag-lanes.md` condition 1): the list is indexed *by lane*, so a lane
    /// in two reservations is a cycle `settle` would walk forever, and a reserved lane that is
    /// also free is the vertex-on-an-edge bug this layout exists to avoid. Both are checked on
    /// every push and every settle in a debug build.
    #[cfg(debug_assertions)]
    fn assert_sole(&self, lane: u32, holder: u32, pool: &Pool) {
        debug_assert_eq!(
            self.held[lane as usize], 1,
            "lane {lane} is in two reservations"
        );
        debug_assert!(
            !pool.free_flag(lane),
            "lane {lane} is reserved for {holder} and free"
        );
    }

    #[cfg(debug_assertions)]
    fn hold(&mut self, lane: u32) {
        // Grow only: `resize` would truncate the higher lanes' counts when a lower lane is
        // re-held, which is exactly what happens when a lane is reused.
        if self.held.len() <= lane as usize {
            self.held.resize(lane as usize + 1, 0);
        }
        self.held[lane as usize] += 1;
    }

    #[cfg(debug_assertions)]
    fn release(&mut self, lane: u32) {
        self.held[lane as usize] -= 1;
    }

    #[cfg(not(debug_assertions))]
    fn hold(&mut self, _lane: u32) {}

    #[cfg(not(debug_assertions))]
    fn release(&mut self, _lane: u32) {}
}

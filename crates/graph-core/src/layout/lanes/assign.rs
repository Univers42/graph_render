//! Lanes: the column each vertex sits in, and the column each edge runs down.
//!
//! Vertices are taken in row order.
//! - A vertex takes the smallest lane its incoming edges reserved for it. If there is none, it
//!   takes the smallest free lane, or opens a new one. The other lanes reserved for it are
//!   freed, because their edges end here.
//! - Its first forward edge carries its own lane on.
//! - A later forward edge shares the target's smallest reserved lane if the target has one,
//!   and otherwise takes a free lane and reserves it.
//! - A vertex with no forward edge frees its lane after its row.
//!
//! A lane inside a reservation is never free, so no vertex is placed on an edge that runs past
//! it.
//!
//! Ponytail (lane choice): lowest-free-lane is greedy, and minimal width is not claimed.
//! Failing input: two lines whose lanes could interleave narrower. Direction: a wider drawing,
//! which is cosmetic. Escape hatch: none needed — every edge is still routed, and the width
//! is reported in `docs/measurements/dag-lanes.md`.

use super::rows::Rows;
use crate::index::Topology;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

pub(super) const NONE: u32 = u32::MAX;

/// Each vertex's lane and each edge's carrying lane.
pub(super) struct Drawing {
    /// `lane[v]`: vertex `v`'s lane.
    pub(super) lane: Vec<u32>,
    /// `carried[e]`: the lane edge `e` runs down between its rows; [`NONE`] for a self-loop.
    pub(super) carried: Vec<u32>,
}

/// Released lanes, smallest first, and how many lanes were ever opened.
struct Pool {
    free: BinaryHeap<Reverse<u32>>,
    width: u32,
    /// Which lanes are in `free`. Debug-only: the one-reservation invariant is a property of
    /// `next` being indexed by lane, and the check has to be O(1) to run on every push.
    #[cfg(debug_assertions)]
    free_flag: Vec<bool>,
}

impl Pool {
    fn take(&mut self) -> u32 {
        if let Some(Reverse(lane)) = self.free.pop() {
            self.mark_taken(lane);
            return lane;
        }
        self.width += 1;
        self.mark_opened(self.width - 1);
        self.width - 1
    }

    fn give(&mut self, lane: u32) {
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
}

/// The lanes reserved for each vertex, one intrusive list per vertex. A lane is in at most one
/// reservation at a time, so `next` is indexed by lane.
struct Reserved {
    head: Vec<u32>,
    next: Vec<u32>,
    min: Vec<u32>,
    /// How many reservations hold each lane. Debug-only: 0 or 1, and the whole correctness
    /// argument rests on it never being 2.
    #[cfg(debug_assertions)]
    held: Vec<u32>,
}

impl Reserved {
    fn push(&mut self, v: u32, lane: u32, pool: &Pool) {
        self.hold(lane);
        #[cfg(debug_assertions)]
        self.assert_sole(lane, v, pool);
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
    /// the caller takes it as `v`'s own lane, but it is no longer *reserved for `v`* — the
    /// first forward edge re-reserves it for its target. A lane the caller never re-reserves
    /// (a vertex with no forward edge) is given to the pool by the caller instead.
    fn settle(&mut self, v: u32, pool: &mut Pool) -> u32 {
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

    /// The one-reservation invariant in code, not only in the doc above
    /// (`docs/decisions/dag-lanes.md` condition 1): `next` is indexed *by lane*, so a lane in
    /// two reservations is a cycle `settle` would walk forever, and a reserved lane that is
    /// also free is the vertex-on-an-edge bug this layout exists to avoid. Both are checked on
    /// every push and every settle in a debug build.
    #[cfg(debug_assertions)]
    fn assert_sole(&self, lane: u32, holder: u32, pool: &Pool) {
        debug_assert_eq!(
            self.held[lane as usize], 1,
            "lane {lane} is in two reservations"
        );
        debug_assert!(
            !pool.free_flag[lane as usize],
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

/// Every non-loop edge under its earlier endpoint, in edge order (a counting sort), each as
/// `(edge, later endpoint)`: `edges[offsets[v]..offsets[v + 1]]`.
struct Forward {
    offsets: Vec<u32>,
    edges: Vec<(u32, u32)>,
}

impl Forward {
    fn of(topology: &Topology, rows: &Rows) -> Self {
        let cols = topology.edges();
        let n = rows.order.len();
        let ends = |e: usize| {
            let (s, t) = (cols.source[e], cols.target[e]);
            if rows.row[s as usize] < rows.row[t as usize] {
                (s, t)
            } else {
                (t, s)
            }
        };
        let kept: Vec<usize> = (0..cols.source.len())
            .filter(|&e| cols.source[e] != cols.target[e])
            .collect();
        let mut offsets = vec![0u32; n + 1];
        for &e in &kept {
            offsets[ends(e).0 as usize + 1] += 1;
        }
        for v in 0..n {
            offsets[v + 1] += offsets[v];
        }
        let mut fill = offsets.clone();
        let mut edges = vec![(0u32, 0u32); offsets[n] as usize];
        for &e in &kept {
            let (early, late) = ends(e);
            edges[fill[early as usize] as usize] = (e as u32, late);
            fill[early as usize] += 1;
        }
        Self { offsets, edges }
    }

    fn of_vertex(&self, v: u32) -> &[(u32, u32)] {
        &self.edges[self.offsets[v as usize] as usize..self.offsets[v as usize + 1] as usize]
    }
}

/// The pool and the reservations, which every placement reads and writes.
struct State {
    pool: Pool,
    reserved: Reserved,
}

impl State {
    /// The lane an edge from a vertex in `lane` to `late` runs down. The vertex's first edge
    /// carries its own lane. A later edge shares `late`'s smallest reserved lane, or takes a
    /// free lane and reserves it for `late`.
    fn carry(&mut self, first: bool, lane: u32, late: u32) -> u32 {
        if first {
            self.reserved.push(late, lane, &self.pool);
            return lane;
        }
        let shared = self.reserved.min[late as usize];
        if shared != NONE {
            return shared;
        }
        let fresh = self.pool.take();
        self.reserved.push(late, fresh, &self.pool);
        fresh
    }
}

impl Drawing {
    /// Every vertex's lane and every edge's carrying lane, in one pass over the rows.
    pub(super) fn of(topology: &Topology, rows: &Rows) -> Self {
        let n = rows.order.len();
        let forward = Forward::of(topology, rows);
        let mut state = State {
            pool: Pool {
                free: BinaryHeap::new(),
                width: 0,
                #[cfg(debug_assertions)]
                free_flag: Vec::new(),
            },
            reserved: Reserved {
                head: vec![NONE; n],
                next: Vec::new(),
                min: vec![NONE; n],
                #[cfg(debug_assertions)]
                held: Vec::new(),
            },
        };
        let mut drawing = Self {
            lane: vec![NONE; n],
            carried: vec![NONE; topology.edges().source.len()],
        };
        for &v in &rows.order {
            drawing.place(v, forward.of_vertex(v), &mut state);
        }
        drawing
    }

    fn place(&mut self, v: u32, out: &[(u32, u32)], state: &mut State) {
        let mut lane = state.reserved.settle(v, &mut state.pool);
        if lane == NONE {
            lane = state.pool.take();
        }
        self.lane[v as usize] = lane;
        for (k, &(edge, late)) in out.iter().enumerate() {
            self.carried[edge as usize] = state.carry(k == 0, lane, late);
        }
        if out.is_empty() {
            state.pool.give(lane);
        }
    }
}

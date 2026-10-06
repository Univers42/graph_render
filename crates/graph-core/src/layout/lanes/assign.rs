//! Lanes: the column each vertex sits in, and the column each edge runs down.
//!
//! Vertices are taken in row order.
//! - A vertex takes the smallest lane its incoming edges reserved for it. If there is none, it
//!   takes the smallest free lane, or opens a new one. The other lanes reserved for it are
//!   freed, because their edges end here.
//!
//! Then each forward edge of the vertex, in dense edge order. Let `S` be the target's smallest
//! reserved lane, if the target has one. The edge **shares `S`** when `S` exists and either
//! `S < lane(v)` — a lower column is already waiting there, so the line bends **left** into it
//! — or `lane(v)` is already carried by an earlier edge of this vertex, which cannot be
//! reserved twice. Otherwise the edge pushes `lane(v)` if no earlier edge took it, or takes
//! the smallest free lane if one did. Either way the lane it chose goes into the target's
//! reservation. (`docs/decisions/dag-lanes-merge.md`, "Addendum: rule D".)
//!
//! After all of the forward edges, `lane(v)` goes back to the pool if no edge took it. That
//! is **after** the edges, never before: freeing it first puts a lane in two reservations on
//! every merge-heavy graph.
//!
//! A lane inside a reservation is never free, so no vertex is placed on an edge that runs past
//! it. The one thing no structural check in the tree can see is the `S < lane(v)` guard above:
//! without it the layout shares on every reserved target and draws rule C, and `assert_sole`,
//! `nothing_sits_on_an_edge` and the hand oracle all still pass. The evidence that the guard
//! is there is `tests/history.rs`'s
//! `a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge`.
//!
//! The two structures this reads — the free pool and the reservations — are `pool.rs` and
//! `reserved.rs`; `rows.rs` has the row order.
//!
//! Ponytail (lane choice): rule D shares a lane only when it is *lower* than the source's own,
//! so a line never bends right into a waiting column — and which column a merge converges into
//! then depends on the order other vertices' edges arrived in. A convention, not a crossing
//! minimiser; lowest-free-lane remains greedy and minimal width is not claimed.
//! Failing input: a merge whose target is already spoken for by a higher lane, where the source
//! keeps its own column and the drawing stays as wide as it was.
//! Direction: a wider drawing, which is cosmetic.
//! Escape hatch: none needed — every edge is still routed, and the width is reported in
//! `docs/measurements/dag-lanes.md`.

mod pool;
mod reserved;

use self::pool::Pool;
use self::reserved::Reserved;
use super::rows::Rows;
use crate::index::Topology;

pub(super) const NONE: u32 = u32::MAX;

/// Each vertex's lane and each edge's carrying lane.
pub(super) struct Drawing {
    /// `lane[v]`: vertex `v`'s lane.
    pub(super) lane: Vec<u32>,
    /// `carried[e]`: the lane edge `e` runs down between its rows; [`NONE`] for a self-loop.
    pub(super) carried: Vec<u32>,
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
    /// Rule D for one forward edge, from a vertex in `lane` to `late`. `own_taken` says
    /// whether an earlier edge of this vertex already took `lane`, which is what tells
    /// "the vertex's own lane is still available" from "the own lane is spoken for".
    ///
    /// The edge shares `late`'s smallest reserved lane when there is one and either it is
    /// lower than `lane` — a column already waiting for `late`, so the line bends left into it
    /// — or `lane` is already spoken for and cannot be reserved twice. Otherwise it pushes
    /// `lane` itself, or takes a free lane when `own_taken` is set, and either way reserves
    /// what it chose for `late`. `docs/decisions/dag-lanes-merge.md`, "Addendum: rule D".
    fn carry(&mut self, lane: u32, late: u32, own_taken: &mut bool) -> u32 {
        let shared = self.reserved.smallest(late);
        if shared != NONE && (shared < lane || *own_taken) {
            return shared;
        }
        let fresh = if *own_taken { self.pool.take() } else { lane };
        *own_taken = true;
        self.reserved.push(late, fresh, &self.pool);
        fresh
    }

    /// Rule D's give-back, run **after** the vertex's forward edges: `lane` returns to the
    /// pool only when no edge took it. Running it first — the reading of "frees it after its
    /// row" that looks natural — puts a lane in two reservations on every merge-heavy graph
    /// (`docs/decisions/dag-lanes-merge.md` condition 1), which the reservations' two checks
    /// both catch in a debug build.
    fn give_back(&mut self, lane: u32) {
        #[cfg(debug_assertions)]
        self.reserved.assert_unheld(lane);
        self.pool.give(lane);
    }
}

impl Drawing {
    /// Every vertex's lane and every edge's carrying lane, in one pass over the rows.
    pub(super) fn of(topology: &Topology, rows: &Rows) -> Self {
        let n = rows.order.len();
        let forward = Forward::of(topology, rows);
        let mut state = State {
            pool: Pool::new(),
            reserved: Reserved::of(n as u32),
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
        let mut own_taken = false;
        for &(edge, late) in out {
            self.carried[edge as usize] = state.carry(lane, late, &mut own_taken);
        }
        if !own_taken {
            state.give_back(lane);
        }
    }
}

//! The hand oracle for `layout.dag.lanes`: the convention of
//! `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md` ("Algorithm") restated
//! with `BTreeSet`s and `BTreeMap`s instead of the layout's `BinaryHeap`s and intrusive
//! lists, then compared bit for bit against the snapshot.
//!
//! **What is shared and what is not.** The convention itself — largest `version` first, tie
//! broken by dense index; the lowest-index unplaced vertex breaking a cycle; a vertex taking
//! its smallest reserved lane, else the smallest free one, else a new one; and **rule D**
//! below for each forward edge — is the thing under test, so both sides necessarily state it.
//! What this file does not share is the code: an ordered set's smallest element and a
//! max-heap's top are different facts computed by different code, the free pool is a
//! `BTreeSet<u32>` against a `BinaryHeap<Reverse<u32>>`, and `reserved` is a map of sets
//! against three `Vec`s indexed by dense node. A transcription error in either would be shared
//! by neither.
//!
//! **Rule D, the lane convention** (`docs/decisions/dag-lanes-merge.md`, "Addendum: rule D",
//! 2026-10-06). For each forward edge of `v`, in dense edge order, let `S` be the target's
//! smallest reserved lane if it has one. The edge **shares `S`** when `S` exists and either
//! `S < lane(v)` — a lower column already waiting for the target, so the line bends *left*
//! into it — or `lane(v)` is already carried by an earlier edge of `v`. Otherwise the edge
//! pushes `lane(v)` if no earlier edge took it, else takes the smallest free lane; either way
//! the lane it chose is pushed into the target's reservation. After all of `v`'s forward
//! edges, `lane(v)` returns to the pool if no edge took it — never before.
//!
//! Because this oracle restates the same rule with different data structures, it cannot be
//! the evidence that the `S < lane(v)` guard is present: a build that shared on every
//! reserved target and called it rule D would agree with this file edge for edge. The
//! evidence is the named test
//! `a_line_bends_left_into_a_column_already_waiting_and_keeps_a_fresh_one_for_its_third_edge`
//! in `crates/graph-core/src/layout/lanes/tests/history.rs`, named in
//! `crates/graph-core/src/registry/lanes.rs`, plus `compare::nothing_sits_on_an_edge`, which
//! reads lanes and rows rather than the assignment.
//!
//! It rebuilds the seed's own model rather than reading it back off the snapshot, which
//! carries positions and not lane membership. `compare` then holds the drawing to the four
//! columns — node `x`, node `y`, polyline `offsets` and `pts`, and notes — and to the
//! structural invariants the spec lists.

mod compare;

use graph_contract::binary::Snapshot;
use graph_core::{REFERENCE_DEGREE, Topology, index_model, seeded_model};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

/// Every vertex's row, lane and every edge's carrying lane: the oracle's whole answer, in
/// one value so `compare` takes three arguments and not five.
pub(super) struct Drawing {
    pub(super) row: Vec<u32>,
    pub(super) lane: Vec<u32>,
    pub(super) carried: Vec<u32>,
}

pub fn lanes(seed: u32, nodes: u32, snapshot: &Snapshot) -> Result<(), String> {
    let (records, edges) = seeded_model(seed, nodes, REFERENCE_DEGREE);
    let topology = index_model(&records, &edges).map_err(|e| format!("reindexing: {e}"))?;
    let row = rows(&topology);
    let (lane, carried) = assign(&topology, &row);
    compare::against(snapshot, &topology, &Drawing { row, lane, carried })
}

/// `f64::total_cmp` as an integer key: the same bit trick `total_cmp` itself applies, so the
/// oracle's ready set is ordered exactly as the layout's heap is, `-0.0` below `0.0` and
/// `-NaN` below `-inf` included.
fn total_key(x: f64) -> i64 {
    let bits = x.to_bits() as i64;
    bits ^ (((bits >> 63) as u64) >> 1) as i64
}

/// The oracle's row order: the ready set is a `BTreeSet` keyed by
/// `(Reverse(total_key(version)), index)`, so its first element is the largest `version` and
/// then the lowest index — the layout heap's top, arrived at by a different route. When the
/// set empties and vertices remain the arcs hold a cycle, and the lowest-index unplaced
/// vertex goes next.
fn rows(t: &Topology) -> Vec<u32> {
    let (n, cols, version) = (t.node_count() as usize, t.edges(), &t.nodes().version);
    let mut succ: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    let mut pending = vec![0u32; n];
    for e in 0..cols.source.len() {
        if cols.directed[e] && cols.source[e] != cols.target[e] {
            succ.entry(cols.source[e]).or_default().push(cols.target[e]);
            pending[cols.target[e] as usize] += 1;
        }
    }
    let key = |v: u32| (Reverse(total_key(version[v as usize])), v);
    let mut ready: BTreeSet<(Reverse<i64>, u32)> = (0..n as u32)
        .filter(|&v| pending[v as usize] == 0)
        .map(key)
        .collect();
    let mut row = vec![u32::MAX; n];
    for r in 0..n as u32 {
        let v = match ready.pop_first() {
            Some((_, v)) => v,
            None => (0..n as u32)
                .find(|&v| row[v as usize] == u32::MAX)
                .expect("one left"),
        };
        row[v as usize] = r;
        if let Some(list) = succ.get(&v) {
            for &w in list {
                pending[w as usize] -= 1;
                if pending[w as usize] == 0 && row[w as usize] == u32::MAX {
                    ready.insert(key(w));
                }
            }
        }
    }
    row
}

/// Every non-loop edge under its earlier endpoint, in dense edge order, as `(edge, later)`.
fn forward(t: &Topology, row: &[u32]) -> BTreeMap<u32, Vec<(u32, u32)>> {
    let cols = t.edges();
    let mut out: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    for e in 0..cols.source.len() {
        let (s, d) = (cols.source[e], cols.target[e]);
        if s == d {
            continue;
        }
        let (early, late) = if row[s as usize] < row[d as usize] {
            (s, d)
        } else {
            (d, s)
        };
        out.entry(early).or_default().push((e as u32, late));
    }
    out
}

/// The oracle's lane assignment, in one pass over the row order.
fn assign(t: &Topology, row: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut out = forward(t, row);
    let mut state = Assign::new();
    let (mut lane, mut carried) = (
        vec![u32::MAX; row.len()],
        vec![u32::MAX; t.edges().source.len()],
    );
    for &v in &order_by_row(row) {
        let own = state.settle(v);
        lane[v as usize] = own;
        let edges = out.remove(&v).unwrap_or_default();
        // Rule D: the give-back runs after the edges, keyed on no edge having taken the
        // vertex's own lane — not on having no forward edge.
        let mut own_taken = false;
        for &(edge, late) in &edges {
            carried[edge as usize] = state.carry(own, late, &mut own_taken);
        }
        if !own_taken {
            state.give(own);
        }
    }
    (lane, carried)
}

/// The vertices in row order: ascending `row`, which the layout gets from Kahn and this from
/// a sort — the same total order by a different route.
fn order_by_row(row: &[u32]) -> Vec<u32> {
    let mut order: Vec<u32> = (0..row.len() as u32).collect();
    order.sort_by_key(|&v| row[v as usize]);
    order
}

/// The free pool and the reservations, both ordered sets — against the layout's
/// `BinaryHeap<Reverse<u32>>` and its three `Vec`s indexed by dense node.
struct Assign {
    free: BTreeSet<u32>,
    width: u32,
    reserved: BTreeMap<u32, BTreeSet<u32>>,
}

impl Assign {
    fn new() -> Self {
        Self {
            free: BTreeSet::new(),
            width: 0,
            reserved: BTreeMap::new(),
        }
    }

    /// The smallest free lane, or a new one when the pool is empty.
    fn take(&mut self) -> u32 {
        match self.free.iter().next().copied() {
            Some(lane) => {
                self.free.remove(&lane);
                lane
            }
            None => {
                self.width += 1;
                self.width - 1
            }
        }
    }

    fn give(&mut self, lane: u32) {
        self.free.insert(lane);
    }

    /// The vertex's own lane: the smallest one its incoming edges reserved for it, else a
    /// free one. Every other lane reserved for it goes back to the pool, because its edges
    /// end here.
    fn settle(&mut self, v: u32) -> u32 {
        let mine = self.reserved.remove(&v).unwrap_or_default();
        let own = match mine.iter().next().copied() {
            Some(lane) => lane,
            None => self.take(),
        };
        for &lane in mine.iter() {
            if lane != own {
                self.free.insert(lane);
            }
        }
        own
    }

    /// Rule D for one forward edge, from a vertex in `own` to `late`. `own_taken` says
    /// whether an earlier edge of this vertex already took `own`.
    ///
    /// The edge shares `late`'s smallest reserved lane when there is one and either it is
    /// lower than `own`, so the line bends left into a column already waiting, or `own` is
    /// already spoken for and cannot be reserved twice. Otherwise it pushes `own` itself, or
    /// takes the smallest free lane when `own_taken` is set, and either way reserves what it
    /// chose for `late`.
    fn carry(&mut self, own: u32, late: u32, own_taken: &mut bool) -> u32 {
        let shared = self
            .reserved
            .get(&late)
            .and_then(|set| set.iter().next().copied());
        if let Some(low) = shared
            && (low < own || *own_taken)
        {
            return low;
        }
        let lane = if *own_taken { self.take() } else { own };
        *own_taken = true;
        self.reserved.entry(late).or_default().insert(lane);
        lane
    }
}

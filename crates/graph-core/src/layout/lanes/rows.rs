//! Rows: a topological order of the directed edges (Kahn).
//!
//! The ready vertices wait in a max-heap whose top is the largest `version`
//! (`f64::total_cmp`), then the lowest dense index. When the heap is empty and vertices
//! remain, the directed edges hold a cycle: the unplaced vertex with the lowest dense index is
//! placed next, found by a forward pointer scan (amortized O(n)). Undirected edges and
//! self-loops impose no order. The whole pass is O((n + m) log n).
//!
//! Ponytail (row tie-break): largest `version` first, then lowest index, is a convention, not
//! a crossing minimiser. Failing input: two independent lines whose `version`s tie
//! interleave by index. Direction: more lane switches, cosmetic. Cycle breaking places the
//! lowest-index unplaced vertex, so more edges are drawn head to tail than a minimum feedback
//! arc set would need. Each one carries note 5, and none is lost. Escape hatch: none needed —
//! every edge is still routed, and the note says which ones run against the rows.

use crate::index::Topology;
use graph_contract::notes::{Note, NoteCode};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

const UNPLACED: u32 = u32::MAX;

/// Every vertex's row and its inverse.
pub(super) struct Rows {
    /// `order[r]`: the vertex at row `r`.
    pub(super) order: Vec<u32>,
    /// `row[v]`: vertex `v`'s row.
    pub(super) row: Vec<u32>,
}

/// A ready vertex. The heap's top is the largest `version`, then the lowest index.
#[derive(Clone, Copy)]
struct Ready {
    version: f64,
    vertex: u32,
}

impl Ord for Ready {
    fn cmp(&self, other: &Self) -> Ordering {
        self.version
            .total_cmp(&other.version)
            .then_with(|| other.vertex.cmp(&self.vertex))
    }
}

impl PartialOrd for Ready {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Ready {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Ready {}

/// The ready heap and the column it is keyed on.
struct Queue<'a> {
    heap: BinaryHeap<Ready>,
    version: &'a [f64],
}

impl Queue<'_> {
    fn push(&mut self, vertex: u32) {
        let version = self.version[vertex as usize];
        self.heap.push(Ready { version, vertex });
    }
}

/// The directed non-loop edges as a CSR (`targets[offsets[v]..offsets[v + 1]]`, edge order),
/// and each vertex's count of predecessors not yet placed.
struct Arcs {
    offsets: Vec<u32>,
    targets: Vec<u32>,
    pending: Vec<u32>,
}

impl Arcs {
    fn of(topology: &Topology) -> Self {
        let n = topology.node_count() as usize;
        let cols = topology.edges();
        let arcs: Vec<usize> = (0..cols.source.len())
            .filter(|&e| cols.directed[e] && cols.source[e] != cols.target[e])
            .collect();
        let mut offsets = vec![0u32; n + 1];
        let mut pending = vec![0u32; n];
        for &e in &arcs {
            offsets[cols.source[e] as usize + 1] += 1;
            pending[cols.target[e] as usize] += 1;
        }
        for v in 0..n {
            offsets[v + 1] += offsets[v];
        }
        let mut fill = offsets.clone();
        let mut targets = vec![0u32; offsets[n] as usize];
        for &e in &arcs {
            let s = cols.source[e] as usize;
            targets[fill[s] as usize] = cols.target[e];
            fill[s] += 1;
        }
        Self {
            offsets,
            targets,
            pending,
        }
    }
}

impl Rows {
    /// Every vertex's row, as the module doc says.
    pub(super) fn of(topology: &Topology) -> Self {
        let n = topology.node_count() as usize;
        let mut arcs = Arcs::of(topology);
        let mut queue = Queue {
            heap: BinaryHeap::with_capacity(n),
            version: &topology.nodes().version,
        };
        for v in (0..n as u32).filter(|&v| arcs.pending[v as usize] == 0) {
            queue.push(v);
        }
        let mut rows = Self {
            order: Vec::with_capacity(n),
            row: vec![UNPLACED; n],
        };
        let mut scan = 0usize;
        while rows.order.len() < n {
            let v = match queue.heap.pop() {
                Some(ready) => ready.vertex,
                None => rows.lowest_unplaced(&mut scan),
            };
            rows.place(v, &mut arcs, &mut queue);
        }
        rows
    }

    fn lowest_unplaced(&self, scan: &mut usize) -> u32 {
        while self.row[*scan] != UNPLACED {
            *scan += 1;
        }
        *scan as u32
    }

    /// Gives `v` the next row. Each successor loses one pending predecessor and becomes
    /// ready at zero, unless a cycle break placed it already.
    fn place(&mut self, v: u32, arcs: &mut Arcs, queue: &mut Queue<'_>) {
        self.row[v as usize] = self.order.len() as u32;
        self.order.push(v);
        let span = arcs.offsets[v as usize] as usize..arcs.offsets[v as usize + 1] as usize;
        for k in span {
            let w = arcs.targets[k] as usize;
            arcs.pending[w] -= 1;
            if arcs.pending[w] == 0 && self.row[w] == UNPLACED {
                queue.push(w as u32);
            }
        }
    }

    /// One note 5 per directed edge whose source has the later row, ascending by edge index.
    ///
    /// **Directed only, deliberately.** `layout.dag.sugiyama`'s own invariant checker notes
    /// every non-loop edge drawn head to tail, directed or not; this layout notes only the
    /// directed ones, because an undirected edge has no head to tail to violate. The
    /// geometry helper reverses the point order so the drawing is still correct either way.
    /// `docs/decisions/dag-lanes.md` condition 6.
    pub(super) fn notes(&self, topology: &Topology) -> Vec<Note> {
        let cols = topology.edges();
        (0..cols.source.len())
            .filter(|&e| {
                let (s, t) = (cols.source[e] as usize, cols.target[e] as usize);
                cols.directed[e] && s != t && self.row[s] > self.row[t]
            })
            .map(|e| Note {
                code: NoteCode::EdgeReversed,
                index: e as u32,
            })
            .collect()
    }
}

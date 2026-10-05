# `layout.dag.lanes` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Register a generic motor layout, `layout.dag.lanes`:
- one row per vertex, in a topological order of the directed edges;
- one lane per line of descent, with lanes reused once free;
- every edge a polyline with at most two interior points.

It runs in O((n + m) log n).

**Architecture:**
- **graph-core.** A new module `layout/lanes` with three parts:
  - `rows.rs`: Kahn's algorithm, ties broken by `version` then index, cycles broken
    deterministically.
  - `assign.rs`: lane assignment with a min-heap pool and intrusive per-vertex reservations.
  - `geometry.rs`: points and polylines.
- **Registry.** A self-contained `Capability` const in `registry/lanes.rs`, appended at the
  end of `LAYOUTS` (append only).
- **graph-cli.** An independent hand oracle on `roundtrip`, and a per-stage hashgate knob that
  mirrors `GM_MUTATE_DAG_DOT_NODES`.

**Tech Stack:** Rust (`std` only, no new dependency), the `scripts/orch/gr` Docker toolchain.

**Spec:** `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md`. Read it before Task 1.
The devil verdict on it is in `docs/decisions/dag-lanes.md`. If that file says BLOCK, stop.
If it says PROCEED-WITH-CONDITIONS, each condition is an extra acceptance criterion.

## Global Constraints

- **The motor knows no data source** (user rule, 2026-10-05). No file under `crates/` may
  contain the words `git`, `commit`, `repository` or `ActivityWatch`. Gate:
  `git grep -n -i -E '\bgit\b|commit|repositor|activitywatch' -- crates/graph-core/src/layout/lanes* crates/graph-core/src/registry/lanes.rs crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs`
  prints nothing.
- **graph-core.**
  - No new dependency, no `unsafe`.
  - `std::collections` only, and never `HashMap`/`HashSet` (D4).
  - No floating-point `mul_add` (D2).
  - Wire integers are `u32`.
- **House limits.**
  - ≤ 40 lines a function, ≤ 4 parameters (not counting `self`), ≤ 300 lines a file, nesting ≤ 3.
  - Every heuristic carries a `Ponytail:` line.
  - No `#[allow]` without a reason line.
- **Toolchain.** Every cargo call is
  `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo ...`. A peer's full gate shares
  the host.
  - Never run a timed gate (`hashgate --seeds 1000`, `mutants.sh`).
  - Never take `~/goinfre/orch/timed.lock`.
- **The registry is append only.** The new entry goes after `dot`'s, the last one, in
  `crates/graph-core/src/registry/layouts.rs`, because the wasm module maps layouts by index.
  `layouts.rs` is already 308 lines: add only the two lines given below. Do not split it in this
  job.
- **Paths you may edit:**
  - `crates/graph-core/src/layout/lanes.rs`, `crates/graph-core/src/layout/lanes/**`,
    `crates/graph-core/src/layout/mod.rs`, `crates/graph-core/src/lib.rs`;
  - `crates/graph-core/src/registry.rs`, `crates/graph-core/src/registry/{lanes.rs,layouts.rs,params.rs,tunable.rs,tests.rs}`,
    `crates/graph-core/src/registry/params/tests/*.rs`;
  - `crates/graph-cli/src/**` (the oracle, the knob and every test that counts or lists
    layouts), `crates/graph-cli/tests/**`;
  - generated files that `graph-cli codegen` rewrites;
  - `docs/measurements/dag-lanes.md`.

  Nothing in `packages/`, `app/`, `server/`, `harness/` or `src/`.

## Review Focus

1. **Every vertex has the same `version`** (the seeded gate model, where every version is 0).
   Rows must fall back to dense index order, deterministically. Pinned by the Task 1 test
   `equal_versions_fall_back_to_index_order`.
2. **A directed cycle** (directed links can form one). It is broken at the unplaced vertex with
   the lowest index, every reversed edge carries note 5, and nothing panics. Pinned by
   `a_directed_cycle_is_broken_at_the_lowest_index_and_noted`.
3. **Many lines converging into one vertex** (a fan-in of 50). No vertex may sit on an edge
   that it does not end, and the lane count stays bounded by the fan-in. Pinned by
   `no_vertex_sits_on_an_edge_it_does_not_end` over a seeded generator with high fan-in.
4. **Parallel edges, and directed mixed with undirected.** Every edge is routed, and undirected
   edges get no note. Pinned by `parallel_and_undirected_edges_are_routed_without_notes`.
5. **An empty graph, a single vertex, and self-loops only.** No panic and empty paths. Pinned by
   `degenerate_graphs_draw`.

---

### Task 1: the layout (graph-core, unregistered)

**Files:**
- Create: `crates/graph-core/src/layout/lanes.rs`, `crates/graph-core/src/layout/lanes/rows.rs`,
  `crates/graph-core/src/layout/lanes/assign.rs`, `crates/graph-core/src/layout/lanes/geometry.rs`,
  `crates/graph-core/src/layout/lanes/tests.rs`
- Modify: `crates/graph-core/src/layout/mod.rs`: add `pub mod lanes;` next to `pub mod sugiyama;`
  (line 31).

**Interfaces:**
- Produces:
  - `pub const ID: &str = "layout.dag.lanes"`
  - `pub struct Lanes` (`impl Stage`, `Params = LanesParams`)
  - `pub struct LanesParams { pub lane_spacing: f32, pub row_spacing: f32 }` (`Default` 1.0 / 1.0)
  - `pub fn run(topology: &Topology, params: &LanesParams) -> Result<Geometry, StageError>`

- [ ] **Step 1: Write the failing tests** in `crates/graph-core/src/layout/lanes/tests.rs`

```rust
use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry, Paths};
use graph_contract::notes::NoteCode;

fn vertex(id: &str, version: f64) -> NodeRecord {
    NodeRecord { version, ..node(id, "") }
}

fn arc(id: &str, source: &str, target: &str) -> EdgeRecord {
    EdgeRecord { directed: true, ..edge(id, source, target) }
}

/// `(x, y, paths, note edge indices)` of one run at unit spacing.
fn drawn(nodes: &[NodeRecord], edges: &[EdgeRecord]) -> (Vec<f32>, Vec<f32>, Paths, Vec<u32>) {
    let topology = index_model(nodes, edges).expect("fits");
    let geometry = run(&topology, &LanesParams::default()).expect("unit spacing is legal");
    let NodeGeometry::Point { x, y } = geometry.nodes else { panic!("not Point nodes") };
    let EdgeGeometry::Polyline(paths) = geometry.edges else { panic!("not Polyline edges") };
    assert!(geometry.notes.iter().all(|n| n.code == NoteCode::EdgeReversed));
    let notes = geometry.notes.iter().map(|n| n.index).collect();
    (x, y, paths, notes)
}

#[test]
fn a_chain_is_one_lane_and_one_row_per_vertex() {
    let n = [vertex("d", 4.0), vertex("c", 3.0), vertex("b", 2.0), vertex("a", 1.0)];
    let e = [arc("dc", "d", "c"), arc("cb", "c", "b"), arc("ba", "b", "a")];
    let (x, y, paths, notes) = drawn(&n, &e);
    assert_eq!(x, [0.0, 0.0, 0.0, 0.0]);
    assert_eq!(y, [0.0, 1.0, 2.0, 3.0]);
    assert_eq!(paths.offsets, [0, 0, 0, 0]);
    assert!(notes.is_empty());
}

#[test]
fn a_branch_and_its_merge_take_two_lanes() {
    // m merges b (first) and c; b and c both descend from a. Rows: m, c (newer), b, a.
    let n = [vertex("a", 1.0), vertex("b", 2.0), vertex("c", 3.0), vertex("m", 4.0)];
    let e = [arc("mb", "m", "b"), arc("mc", "m", "c"), arc("ba", "b", "a"), arc("ca", "c", "a")];
    let (x, y, paths, notes) = drawn(&n, &e);
    assert_eq!(x, [0.0, 0.0, 1.0, 0.0], "a, b, c, m");
    assert_eq!(y, [3.0, 2.0, 1.0, 0.0], "a, b, c, m");
    assert_eq!(paths.offsets, [0, 0, 1, 1, 2]);
    assert_eq!(paths.pts, [1.0, 0.5, 1.0, 2.5]);
    assert!(notes.is_empty());
}

#[test]
fn a_directed_cycle_is_broken_at_the_lowest_index_and_noted() {
    let n = [vertex("a", 0.0), vertex("b", 0.0), vertex("c", 0.0)];
    let e = [arc("ab", "a", "b"), arc("bc", "b", "c"), arc("ca", "c", "a")];
    let (x, y, paths, notes) = drawn(&n, &e);
    assert_eq!(y, [0.0, 1.0, 2.0]);
    assert_eq!(x, [0.0, 0.0, 0.0]);
    assert_eq!(notes, [2], "c -> a runs against the rows");
    assert_eq!(paths.offsets, [0, 0, 0, 2]);
    assert_eq!(paths.pts, [1.0, 1.5, 1.0, 0.5], "source c to target a");
}

#[test]
fn equal_versions_fall_back_to_index_order() {
    let n = [vertex("p", 0.0), vertex("q", 0.0), vertex("r", 0.0)];
    let (_, y, _, _) = drawn(&n, &[]);
    assert_eq!(y, [0.0, 1.0, 2.0]);
}

#[test]
fn parallel_and_undirected_edges_are_routed_without_notes() {
    let n = [vertex("a", 1.0), vertex("b", 2.0)];
    let e = [edge("u", "a", "b"), arc("d1", "b", "a"), arc("d2", "b", "a")];
    let (_, y, paths, notes) = drawn(&n, &e);
    assert_eq!(y, [1.0, 0.0], "b is newer");
    assert_eq!(paths.offsets.len(), 4);
    assert!(notes.is_empty(), "an undirected edge is never reversed: {notes:?}");
}

#[test]
fn degenerate_graphs_draw() {
    let (x, _, paths, _) = drawn(&[], &[]);
    assert!(x.is_empty() && paths.offsets == [0]);
    let (x, _, paths, notes) = drawn(&[vertex("a", 0.0)], &[arc("aa", "a", "a")]);
    assert_eq!(x, [0.0]);
    assert_eq!(paths.offsets, [0, 0]);
    assert!(notes.is_empty());
}

#[test]
fn spacing_scales_both_axes_and_a_bad_one_is_refused() {
    let n = [vertex("m", 2.0), vertex("a", 1.0), vertex("b", 1.0)];
    let e = [arc("ma", "m", "a"), arc("mb", "m", "b")];
    let t = index_model(&n, &e).expect("fits");
    let wide = LanesParams { lane_spacing: 2.0, row_spacing: 3.0 };
    let NodeGeometry::Point { x, y } = run(&t, &wide).expect("legal").nodes else { panic!() };
    assert_eq!((x[2], y[2]), (2.0, 6.0), "b: lane 1, row 2");
    for bad in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(run(&t, &LanesParams { lane_spacing: bad, row_spacing: 1.0 }).is_err());
        assert!(run(&t, &LanesParams { lane_spacing: 1.0, row_spacing: bad }).is_err());
    }
}

/// A seeded history-shaped DAG: vertex `i` points at one or two later vertices, with a fan-in
/// up to 50 on every 97th vertex. Deterministic (an LCG, no clock, no `rand`).
fn history(n: u32, seed: u64) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let mut state = seed;
    let mut next = |bound: u32| {
        state = state.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) % u64::from(bound.max(1))) as u32
    };
    let ids: Vec<String> = (0..n).map(|i| format!("v{i}")).collect();
    let nodes = ids.iter().enumerate().map(|(i, id)| vertex(id, f64::from(n - i as u32))).collect();
    let mut edges = Vec::new();
    for i in 0..n.saturating_sub(1) {
        let parents = if i % 97 == 0 { 50 } else { 1 + next(2) };
        for k in 0..parents {
            let p = (i + 1 + next(40)).min(n - 1);
            edges.push(arc(&format!("e{i}_{k}"), &ids[i as usize], &ids[p as usize]));
        }
    }
    (nodes, edges)
}

#[test]
fn no_vertex_sits_on_an_edge_it_does_not_end() {
    for seed in 1..=8 {
        let (n, e) = history(2_000, seed);
        let t = index_model(&n, &e).expect("fits");
        let (x, y, paths, notes) = drawn(&n, &e);
        assert!(notes.is_empty(), "seed {seed}: a DAG reverses nothing");
        let cols = t.edges();
        let mut at: std::collections::BTreeMap<(u32, u32), u32> = Default::default();
        for (v, (&px, &py)) in x.iter().zip(&y).enumerate() {
            at.insert((px as u32, py as u32), v as u32);
        }
        for (edge, (&s, &d)) in cols.source.iter().zip(&cols.target).enumerate() {
            let (s, d) = (s as usize, d as usize);
            assert!(y[s] < y[d], "seed {seed} edge {edge}: rows run source to target");
            let span = paths.offsets[edge] as usize..paths.offsets[edge + 1] as usize;
            let lane = span.clone().next().map_or(x[s], |p| paths.pts[2 * p]);
            for row in (y[s] as u32 + 1)..(y[d] as u32) {
                let sitting = at.get(&(lane as u32, row));
                assert!(sitting.is_none(), "seed {seed} edge {edge}: vertex {sitting:?} on lane {lane} row {row}");
            }
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo test -p graph-core lanes`
Expected: compile errors: `run`, `LanesParams` and `lanes` do not exist.

- [ ] **Step 3: Write `crates/graph-core/src/layout/lanes.rs`**

```rust
//! `layout.dag.lanes`: one row per vertex in a topological order of the directed edges, each
//! line of descent kept in a lane, a lane reused once it is free, and every edge a polyline of
//! at most two interior points. No dummy vertices and no crossing reduction, so the cost is
//! O((n + m) log n) with no budget to run out of.
//! Spec: `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md`.

mod assign;
mod geometry;
mod rows;
#[cfg(test)]
mod tests;

use super::Geometry;
use crate::index::Topology;
use crate::stage::{Stage, StageError};
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};

/// The registry id.
pub const ID: &str = "layout.dag.lanes";

/// The lanes stage.
#[derive(Debug, Clone, Copy)]
pub struct Lanes;

/// The lanes stage's parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LanesParams {
    /// X distance between adjacent lanes. Finite and above 0.
    pub lane_spacing: f32,
    /// Y distance between adjacent rows. Finite and above 0.
    pub row_spacing: f32,
}

impl Default for LanesParams {
    fn default() -> Self {
        Self { lane_spacing: 1.0, row_spacing: 1.0 }
    }
}

impl Stage for Lanes {
    type Params = LanesParams;
    const ID: &'static str = ID;

    fn run(topology: &Topology, params: &LanesParams) -> Result<Geometry, StageError> {
        run(topology, params)
    }
}

/// Rows, then lanes, then the geometry. Fails only on a spacing that is not finite and
/// above 0: every `Topology`, the empty one included, has a lanes drawing.
pub fn run(topology: &Topology, params: &LanesParams) -> Result<Geometry, StageError> {
    legal("lane_spacing", params.lane_spacing)?;
    legal("row_spacing", params.row_spacing)?;
    let rows = rows::Rows::of(topology);
    let drawing = assign::Drawing::of(topology, &rows);
    let (x, y) = geometry::positions(&drawing, &rows, params);
    let paths = geometry::paths(&drawing, topology, &rows, params);
    Ok(Geometry::planar(
        NodeGeometry::Point { x, y },
        EdgeGeometry::Polyline(paths),
        rows.notes(topology),
    ))
}

fn legal(name: &'static str, value: f32) -> Result<(), StageError> {
    if value.is_finite() && value > 0.0 {
        return Ok(());
    }
    Err(StageError::Param { name, rule: "finite and above 0" })
}
```

- [ ] **Step 4: Write `crates/graph-core/src/layout/lanes/rows.rs`**

```rust
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
//! arc set would need. Each one carries note 5, and none is lost.

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
        Self { offsets, targets, pending }
    }
}

impl Rows {
    /// Every vertex's row, as the module doc says.
    pub(super) fn of(topology: &Topology) -> Self {
        let n = topology.node_count() as usize;
        let mut arcs = Arcs::of(topology);
        let mut queue = Queue { heap: BinaryHeap::with_capacity(n), version: &topology.nodes().version };
        for v in (0..n as u32).filter(|&v| arcs.pending[v as usize] == 0) {
            queue.push(v);
        }
        let mut rows = Self { order: Vec::with_capacity(n), row: vec![UNPLACED; n] };
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
    pub(super) fn notes(&self, topology: &Topology) -> Vec<Note> {
        let cols = topology.edges();
        (0..cols.source.len())
            .filter(|&e| {
                let (s, t) = (cols.source[e] as usize, cols.target[e] as usize);
                cols.directed[e] && s != t && self.row[s] > self.row[t]
            })
            .map(|e| Note { code: NoteCode::EdgeReversed, index: e as u32 })
            .collect()
    }
}
```

- [ ] **Step 5: Write `crates/graph-core/src/layout/lanes/assign.rs`**

```rust
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
//! which is cosmetic. Every edge is still routed.

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
}

impl Pool {
    fn take(&mut self) -> u32 {
        if let Some(Reverse(lane)) = self.free.pop() {
            return lane;
        }
        self.width += 1;
        self.width - 1
    }

    fn give(&mut self, lane: u32) {
        self.free.push(Reverse(lane));
    }
}

/// The lanes reserved for each vertex, one intrusive list per vertex. A lane is in at most one
/// reservation at a time, so `next` is indexed by lane.
struct Reserved {
    head: Vec<u32>,
    next: Vec<u32>,
    min: Vec<u32>,
}

impl Reserved {
    fn push(&mut self, v: u32, lane: u32) {
        if self.next.len() <= lane as usize {
            self.next.resize(lane as usize + 1, NONE);
        }
        self.next[lane as usize] = self.head[v as usize];
        self.head[v as usize] = lane;
        self.min[v as usize] = self.min[v as usize].min(lane);
    }

    /// Empties `v`'s reservation and returns its smallest lane, or [`NONE`]. Every other lane
    /// in it goes back to `pool`.
    fn settle(&mut self, v: u32, pool: &mut Pool) -> u32 {
        let keep = self.min[v as usize];
        let mut lane = self.head[v as usize];
        while lane != NONE {
            let next = self.next[lane as usize];
            if lane != keep {
                pool.give(lane);
            }
            lane = next;
        }
        self.head[v as usize] = NONE;
        self.min[v as usize] = NONE;
        keep
    }
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
            if rows.row[s as usize] < rows.row[t as usize] { (s, t) } else { (t, s) }
        };
        let kept: Vec<usize> = (0..cols.source.len()).filter(|&e| cols.source[e] != cols.target[e]).collect();
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
            self.reserved.push(late, lane);
            return lane;
        }
        let shared = self.reserved.min[late as usize];
        if shared != NONE {
            return shared;
        }
        let fresh = self.pool.take();
        self.reserved.push(late, fresh);
        fresh
    }
}

impl Drawing {
    /// Every vertex's lane and every edge's carrying lane, in one pass over the rows.
    pub(super) fn of(topology: &Topology, rows: &Rows) -> Self {
        let n = rows.order.len();
        let forward = Forward::of(topology, rows);
        let mut state = State {
            pool: Pool { free: BinaryHeap::new(), width: 0 },
            reserved: Reserved { head: vec![NONE; n], next: Vec::new(), min: vec![NONE; n] },
        };
        let mut drawing = Self { lane: vec![NONE; n], carried: vec![NONE; topology.edges().source.len()] };
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
```

- [ ] **Step 6: Write `crates/graph-core/src/layout/lanes/geometry.rs`**

```rust
//! Points and polylines. A vertex sits at `(lane · lane_spacing, row · row_spacing)`. An edge
//! carried in lane `l` bends into `l` half a row after its earlier end when `l` is not that
//! end's lane, and out of `l` half a row before its later end when `l` is not that end's lane.
//! Its interior points run from the edge's own source to its own target. A self-loop has none,
//! the same convention as `layout.dag.sugiyama`.

use super::LanesParams;
use super::assign::Drawing;
use super::rows::Rows;
use crate::index::Topology;
use graph_contract::geometry::Paths;

/// Every vertex's `(x, y)`, in node order.
pub(super) fn positions(drawing: &Drawing, rows: &Rows, params: &LanesParams) -> (Vec<f32>, Vec<f32>) {
    let x = drawing.lane.iter().map(|&l| l as f32 * params.lane_spacing).collect();
    let y = rows.row.iter().map(|&r| r as f32 * params.row_spacing).collect();
    (x, y)
}

/// Every edge's interior points, CSR-shaped like `layout.dag.sugiyama`'s.
pub(super) fn paths(drawing: &Drawing, topology: &Topology, rows: &Rows, params: &LanesParams) -> Paths {
    let cols = topology.edges();
    let m = cols.source.len();
    let mut offsets = Vec::with_capacity(m + 1);
    let mut pts = Vec::with_capacity(2 * m);
    offsets.push(0);
    for e in 0..m {
        let (s, t) = (cols.source[e], cols.target[e]);
        if s != t {
            let route = Route { source: s, target: t, lane: drawing.carried[e] };
            push_route(&mut pts, route, (drawing, rows), params);
        }
        offsets.push((pts.len() / 2) as u32);
    }
    Paths { offsets, pts }
}

/// One edge: its endpoints and the lane carrying it.
struct Route {
    source: u32,
    target: u32,
    lane: u32,
}

fn push_route(pts: &mut Vec<f32>, route: Route, (drawing, rows): (&Drawing, &Rows), params: &LanesParams) {
    let (rs, rt) = (rows.row[route.source as usize], rows.row[route.target as usize]);
    let (early, late) = if rs < rt { (route.source, route.target) } else { (route.target, route.source) };
    let x = route.lane as f32 * params.lane_spacing;
    let mut points = [(0.0f32, 0.0f32); 2];
    let mut count = 0;
    if route.lane != drawing.lane[early as usize] {
        points[count] = (x, (rs.min(rt) as f32 + 0.5) * params.row_spacing);
        count += 1;
    }
    if route.lane != drawing.lane[late as usize] {
        points[count] = (x, (rs.max(rt) as f32 - 0.5) * params.row_spacing);
        count += 1;
    }
    let points = &mut points[..count];
    if early != route.source {
        points.reverse();
    }
    for &(px, py) in points.iter() {
        pts.push(px);
        pts.push(py);
    }
}
```

- [ ] **Step 7: Run the tests to verify they pass**

Run: `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo test -p graph-core lanes`
Expected: 8 tests pass. If `records::build` is private to another module, use the path
`sugiyama/tests.rs:3` uses: it imports `crate::records::build::{edge, node}` the same way.

- [ ] **Step 8: Run fmt and clippy**, then commit

Run:
- `scripts/orch/gr cargo fmt --all`
- `CARGO_BUILD_JOBS=3 scripts/orch/gr cargo clippy -p graph-core --all-targets -- -D warnings`

Expected: exit 0 for both.

Commit:
`git add crates/graph-core/src/layout/lanes.rs crates/graph-core/src/layout/lanes crates/graph-core/src/layout/mod.rs && git -c user.name=LESdylan -c user.email=dev.pro.photo@gmail.com commit -qm updated`
(If the job's permission layer denies `git commit`, leave the change staged and say so.)

### Task 2: register it

**Files:**
- Create: `crates/graph-core/src/registry/lanes.rs`
- Modify:
  - `crates/graph-core/src/registry.rs`: `mod lanes;` beside `mod grid;` (line 30), and
    `pub use lanes::LANES_CEILING;`
  - `crates/graph-core/src/registry/layouts.rs`: append two lines after `dot`'s entry, the
    last one before `];`
  - `crates/graph-core/src/registry/tunable.rs`: a `tunable!` block
  - `crates/graph-core/src/registry/params.rs`: `published!`
  - `crates/graph-core/src/registry/params/tests/schema.rs`: `every_published!`
  - `crates/graph-core/src/lib.rs`: `pub use layout::lanes::{Lanes, LanesParams};` beside line 61
  - `crates/graph-core/src/registry/tests.rs`: one test

**Interfaces:**
- Consumes: Task 1's `lanes::{ID, Lanes, LanesParams}`.
- Produces: `registry::find("layout.dag.lanes")` and `registry::LANES_CEILING: u64`.

- [ ] **Step 1: Write the failing registry test**, appended to `crates/graph-core/src/registry/tests.rs`

```rust
#[test]
fn lanes_declares_point_nodes_polyline_edges_and_its_two_spacings() {
    let lanes = find("layout.dag.lanes").expect("registered");
    assert_eq!(lanes.meta.nodes, NodeGeometryKind::Point);
    assert_eq!(lanes.meta.edges, EdgeGeometryKind::Polyline);
    let names: Vec<_> = lanes.params.specs.iter().map(|spec| spec.name).collect();
    assert_eq!(names, ["lane_spacing", "row_spacing"]);
    assert_eq!(LAYOUTS.last().map(|c| c.id), Some("layout.dag.lanes"), "appended last");
}
```

Check the imports that the existing `sugiyama_declares_polyline_edges_and_the_reference_dummy_budget`
test (`registry/tests.rs:97`) uses, and the field name of a param spec in `params.rs`. If it is
not `name`, use what `params.rs` declares.

- [ ] **Step 2: Run it to verify it fails**

Run: `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo test -p graph-core lanes_declares`
Expected: FAIL. `find` returns `None`.

- [ ] **Step 3: Write `crates/graph-core/src/registry/lanes.rs`**

```rust
//! `layout.dag.lanes`'s registry entry, self-contained like `three_d/random3d.rs`'s, so
//! `layouts.rs` only appends one line (append only: the wasm module maps a layout by index).

use super::{Capability, Metadata, params, run_default};
use crate::layout::lanes::{ID, Lanes};
use graph_contract::geometry::{EdgeGeometryKind, NodeGeometryKind};

/// Vertices past which the lanes layout has not been measured. Ponytail (scale_ceiling):
/// labelled from the native bench in `docs/measurements/dag-lanes.md` (Task 4), not a proof;
/// the algorithm itself has no budget and degrades in time only.
pub const LANES_CEILING: u64 = 1_000_000;

const LANES: Metadata = Metadata {
    tier: 1,
    stage: "layout",
    nodes: NodeGeometryKind::Point,
    edges: EdgeGeometryKind::Polyline,
    oracle: "hand oracle on roundtrip (graph-cli snapshot_cmd::hand_oracles::lanes): the stated \
convention restated with ordered sets instead of heaps, compared bit for bit per seed; lane \
width measured in docs/measurements/dag-lanes.md",
    complexity: "O((n + m) log n): one counting sort of the directed edges, Kahn with a ready \
heap, one counting sort of the edges by their earlier row, and a min-heap of free lanes; \
O(n + m) memory, no dummy vertices",
    scale_ceiling: LANES_CEILING,
    degradation: "none in routing: every edge has at most two interior points and none is left \
unrouted. Width grows with the number of lines alive at once, up to n on an antichain (the \
seeded model's undirected graph draws wide); time stays O((n + m) log n)",
    ponytail: "Ponytail (row tie-break): ready vertices go largest version first, then lowest \
index; a convention, not a crossing minimiser; equal versions interleave by index, which adds \
lane switches, cosmetic. Ponytail (cycle breaking): a directed cycle is broken at the \
lowest-index unplaced vertex, so more edges than a minimum feedback arc set are drawn head to \
tail, each with note 5, none lost. Ponytail (lane choice): lowest-free-lane is greedy; minimal \
width is not claimed; a wider drawing is cosmetic",
};

/// `LAYOUTS` appends this after `layout.dag.dot`.
pub(in crate::registry) const LANES_LAYOUT: Capability = Capability {
    id: ID,
    run: run_default::<Lanes>,
    params: &params::LANES,
    meta: LANES,
};
```

If `run_default` or `params` are not visible from a child module, make the narrowest visibility
change in `registry.rs` that compiles: `pub(in crate::registry)`.

- [ ] **Step 4: Wire it in**
  - `registry/layouts.rs`, after `dot`'s `Capability { ... },` and before `];`:
    ```rust
        // ---- dag-lanes: one row per vertex, reused lanes, appended last (append only).
        super::lanes::LANES_LAYOUT,
    ```
  - `registry/tunable.rs`, after the `SugiyamaParams` block (line 196):
    ```rust
    tunable!(LanesParams, true, {
        lane_spacing: f32, Float, 0.0625, 1024.0, 1.0, 0.125,
            "x distance between adjacent lanes";
        row_spacing: f32, Float, 0.0625, 1024.0, 1.0, 0.125,
            "y distance between adjacent rows";
    });
    ```
    Also add `use crate::layout::lanes::LanesParams;` beside `use crate::layout::sugiyama::SugiyamaParams;`
    (line 26).
  - `registry/params.rs`: `use crate::layout::lanes::Lanes;` beside the Sugiyama import, and
    `published!(LANES, Lanes);` after `published!(SUGIYAMA, Sugiyama);`.
  - `registry/params/tests/schema.rs`: `use crate::layout::lanes::LanesParams;` and
    `$each!(LanesParams);` after `$each!(SugiyamaParams);`.
  - `lib.rs`: `pub use layout::lanes::{Lanes, LanesParams};` after line 61.

- [ ] **Step 5: Run the graph-core suite**

Run: `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo test -p graph-core --no-fail-fast`
Expected: PASS.
- A test that counts registered layouts or lists ids gets the new count or id added. Change
  nothing else in such a test.
- Run `scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown`. Expected:
  exit 0.
- Commit, as in Task 1.

### Task 3: the hand oracle and the per-stage knob (graph-cli)

**Files:**
- Create: `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs`
- Modify:
  - `crates/graph-cli/src/snapshot_cmd/hand_oracles.rs`: `mod lanes; pub use lanes::lanes;`
  - `crates/graph-cli/src/snapshot_cmd/roundtrip/seed.rs:66-85`: one arm each in
    `hand_oracle` and `convention`
  - `crates/graph-cli/src/snapshot_cmd/roundtrip.rs`: a `lanes: Vec<String>` field in
    `Findings` (line 30), its entry in `pass` (line 75), and
    `"layout.dag.lanes": hand(found.lanes.len())` in `body`
  - `crates/graph-cli/src/capabilities/registry/layout_row.rs:48`:
    `const ROUNDTRIP_LAYOUTS: [&str; 5]` gains `"layout.dag.lanes"`
  - the knob: every place `DagDotNodes` / `DOT_LAYOUT_STAGES` / `GM_MUTATE_DAG_DOT_NODES`
    appears. Find them with
    `git grep -n 'DagDotNodes\|DOT_LAYOUT_STAGES\|GM_MUTATE_DAG_DOT_NODES\|dag-dot-nodes' -- crates/graph-cli`.
    Add the lanes twin beside each:
    - `DagLanesNodes`
    - `LANES_LAYOUT_STAGES` (`env: "GM_MUTATE_DAG_LANES_NODES"`, stage `"layout.dag.lanes"`,
      record `"hashgate-control-dag-lanes-nodes"`)
    - a `crates/graph-cli/src/hashgate/tests/knob/lanes.rs` copied from `knob/dot.rs` with the
      names replaced
    - the knob list in `crates/graph-cli/tests/common/mod.rs`, if `GM_MUTATE_DAG_DOT_NODES` is
      listed there

**Interfaces:**
- Consumes: `graph_core::{Topology, index_model, seeded_model, REFERENCE_DEGREE}`, and the
  snapshot's `Point` / `Polyline` columns and notes.
- Produces: `pub fn lanes(seed: u32, nodes: u32, snapshot: &Snapshot) -> Result<(), String>`.

- [ ] **Step 1: Write the failing oracle tests**, in `crates/graph-cli/src/snapshot_cmd/hand_oracles/tests.rs`
  (or a sibling `tests/lanes.rs` if that file would pass 300 lines)

```rust
#[test]
fn lanes_matches_its_own_layout_and_names_a_moved_vertex() {
    for seed in 0..4 {
        let nodes = graph_core::gate_node_count(seed).min(400);
        let snapshot = crate::snapshot_cmd::pipeline_snapshot(seed, nodes, "dag.lanes");
        assert_eq!(super::lanes(seed, nodes, &snapshot), Ok(()), "seed {seed}");
        let moved = with_x(&snapshot, 0, 0.5);
        let err = super::lanes(seed, nodes, &moved).expect_err("moved half a lane");
        assert!(err.contains("node 0"), "{err}");
    }
}
```

`pipeline_snapshot` and `with_x` are whatever the neighbouring grid tests
(`snapshot_cmd/tests.rs:120-150`, `grid_with`) already use to build a snapshot for one layout
and to perturb one column. Reuse those helpers rather than writing new ones. If no
`pipeline(seed, nodes, name)` helper is reachable from the test module, use the one
`roundtrip/seed.rs` imports (`super::super::pipeline`).

- [ ] **Step 2: Run it to verify it fails**

Run: `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo test -p graph-cli lanes_matches`
Expected: FAIL to compile (`lanes` not found).

- [ ] **Step 3: Write `crates/graph-cli/src/snapshot_cmd/hand_oracles/lanes.rs`**, an
  independent restatement with ordered sets

```rust
//! The hand oracle for `layout.dag.lanes`: the convention of
//! `docs/superpowers/specs/2026-10-05-dag-lanes-layout-design.md` ("Algorithm") restated with
//! `BTreeSet`s and `BTreeMap`s instead of the layout's heaps and intrusive lists, then compared
//! bit for bit with the snapshot: node `x`/`y`, polyline offsets and points, notes.

use graph_contract::binary::Snapshot;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::notes::NoteCode;
use graph_core::{REFERENCE_DEGREE, Topology, index_model, seeded_model};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

pub fn lanes(seed: u32, nodes: u32, snapshot: &Snapshot) -> Result<(), String> {
    let (records, edges) = seeded_model(seed, nodes, REFERENCE_DEGREE);
    let topology = index_model(&records, &edges).map_err(|e| format!("reindexing: {e}"))?;
    let row = rows(&topology);
    let (lane, carried) = assign(&topology, &row);
    compare(snapshot, &topology, (&row, &lane, &carried))
}

/// `f64::total_cmp` as an integer key (the standard bit trick total_cmp itself uses).
fn total_key(x: f64) -> i64 {
    let bits = x.to_bits() as i64;
    bits ^ (((bits >> 63) as u64) >> 1) as i64
}

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
    let mut ready: BTreeSet<(Reverse<i64>, u32)> = (0..n as u32).filter(|&v| pending[v as usize] == 0).map(key).collect();
    let mut row = vec![u32::MAX; n];
    for r in 0..n as u32 {
        let v = match ready.pop_first() {
            Some((_, v)) => v,
            None => (0..n as u32).find(|&v| row[v as usize] == u32::MAX).expect("one left"),
        };
        row[v as usize] = r;
        for &w in succ.get(&v).map_or(&[][..], Vec::as_slice) {
            pending[w as usize] -= 1;
            if pending[w as usize] == 0 && row[w as usize] == u32::MAX {
                ready.insert(key(w));
            }
        }
    }
    row
}

fn assign(t: &Topology, row: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let cols = t.edges();
    let mut order: Vec<u32> = (0..row.len() as u32).collect();
    order.sort_by_key(|&v| row[v as usize]);
    let mut out: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    for e in 0..cols.source.len() {
        let (s, d) = (cols.source[e], cols.target[e]);
        if s == d { continue; }
        let (early, late) = if row[s as usize] < row[d as usize] { (s, d) } else { (d, s) };
        out.entry(early).or_default().push((e as u32, late));
    }
    let (mut free, mut width) = (BTreeSet::<u32>::new(), 0u32);
    let mut take = |free: &mut BTreeSet<u32>| free.pop_first().unwrap_or_else(|| { width += 1; width - 1 });
    let mut reserved: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let (mut lane, mut carried) = (vec![u32::MAX; row.len()], vec![u32::MAX; cols.source.len()]);
    for v in order {
        let mine = reserved.remove(&v).unwrap_or_default();
        let own = mine.first().copied().unwrap_or_else(|| take(&mut free));
        free.extend(mine.iter().copied().filter(|&l| l != own));
        lane[v as usize] = own;
        let edges = out.remove(&v).unwrap_or_default();
        for (k, &(e, late)) in edges.iter().enumerate() {
            let set = reserved.entry(late).or_default();
            carried[e as usize] = if k == 0 { set.insert(own); own } else if let Some(&low) = set.first() { low } else { let l = take(&mut free); set.insert(l); l };
        }
        if edges.is_empty() { free.insert(own); }
    }
    (lane, carried)
}
```

Then write `compare(snapshot, topology, (row, lane, carried)) -> Result<(), String>`. It
rebuilds the expected `x`, `y` and polyline points with the rules of Task 1's
`geometry.rs`: `lane as f32 * 1.0`, `row as f32 * 1.0`, `(early_row as f32 + 0.5) * 1.0`,
`(late_row as f32 - 0.5) * 1.0`, reversed when the source is the later end. It also rebuilds
the expected notes (`EdgeReversed` for each directed non-loop edge whose source has the later
row, ascending). It compares them with `snapshot.parts()` with `to_bits()`. The first
difference returns `Err` naming the node (`"node {v} at ({gx}, {gy}), the convention puts it
at ({wx}, {wy})"`), the edge, or the note.

**Borrow note.** If the `take` closure's `width` capture fights the borrow checker, make
`width` a field of a two-field struct with a `take` method. Keep the BTree types: they are
what makes this oracle independent of the layout's heaps.

- [ ] **Step 4: Wire it in** as the Files list says, then run

Run: `CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo test -p graph-cli --no-fail-fast`
Expected: PASS. Update every test that pins the layout count, the roundtrip JSON body
(`roundtrip/tests.rs:98`), `snapshot_cmd/tests/names.rs`, `capabilities/tests/stages.rs` or the
knob coverage table: each gains exactly the new id or count, and nothing else changes.

- [ ] **Step 5: Commit**, as in Task 1.

### Task 4: gates, codegen, the measurement

- [ ] **Step 1: Run the merge floor and the gate rows.** Paste each exit code into the report:

```
scripts/orch/gr cargo fmt --all --check                                                   -> 0
CARGO_BUILD_JOBS=3 scripts/orch/gr cargo clippy --workspace --all-targets -- -D warnings  -> 0
CARGO_BUILD_JOBS=3 RUST_TEST_THREADS=3 scripts/orch/gr cargo test --workspace --no-fail-fast -> 0
scripts/orch/gr cargo build -p graph-core --target wasm32-unknown-unknown                 -> 0
scripts/orch/gr cargo run -q -p graph-cli -- codegen --check                              -> 0 (else run `codegen` once, commit the regenerated files, re-check)
scripts/orch/gr cargo run -q -p graph-cli -- capabilities --check                         -> 0
scripts/orch/gr cargo run -q -p graph-cli -- roundtrip --seeds 100                        -> 0
scripts/orch/gr cargo run -q -p graph-cli -- hashgate --seeds 8                           -> 0
scripts/orch/gr -e GM_MUTATE_REFERENCE_DEGREE=9 cargo run -q -p graph-cli -- hashgate --seeds 8 -> non-zero
scripts/orch/gr -e GM_MUTATE_DAG_LANES_NODES=1 cargo run -q -p graph-cli -- hashgate --seeds 8   -> non-zero, naming layout.dag.lanes only
```

The no-data-source grep from Global Constraints must print nothing.

- [ ] **Step 2: Native bench.** Wait until `pgrep -f develop-full.rows` finds nothing (apart
  from opencode), `free -g` shows ≥ 12 GB available and the 1-minute load is < 14. Then run
  `flock ~/goinfre/orch/bench.lock env GR_MEM=12g scripts/orch/gr cargo run -q --release -p graph-cli -- bench --layout layout.dag.lanes --n 10000,100000,1000000 --repeat 3`.
  - Three rounds, medians, load printed.
  - If 1M exceeds 1 s native, set `LANES_CEILING` to the largest measured n under 1 s, and
    say so.

- [ ] **Step 3: Write `docs/measurements/dag-lanes.md`.** Include:
  - the gate table with exit codes;
  - the bench table with load per run;
  - this Caveat: "the host is shared; medians under load are upper bounds";
  - "what it does not do", copied from the spec.

  Real-history inputs are measured by the git plugin job, not here, so the crate stays unaware
  of them. Commit.

- [ ] **Step 4: Server rows.** Run `scripts/orch/gate.sh target/gate-dag-lanes scripts/orch/rows/svc-floor.rows`
  and paste `summary.txt`. A red caps or digest row is expected: a new layout needs a caps row
  and digest rows. Do **not** edit `server/` or `docs/measurements/service-caps.tsv`. Put the
  row names under "decisions needed" for the orchestrator to send to their owner.

## Return block

```
status: done | partial | blocked
verdict conditions: <each condition from docs/decisions/dag-lanes.md -> how met>
changed: <files>
commands: <each Task 4 gate -> rc>
bench: 10k / 100k / 1M medians (ms), load
svc-floor: <rows red and why>
deviations: <none | list>
decisions needed: <none | list>
```

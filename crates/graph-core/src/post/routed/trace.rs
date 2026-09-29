//! Walking a distance field back to its source (`prompts/phase-08-post-routing-bundling.md`
//! step 3: "the tie-break must be total and explicit (by cell index)").
//!
//! Reference: `SciGraphs/engine/scigraphs_engine/bundling/routed.py::_trace_numpy`. The
//! step minimises `dist[n] + w(n, x)`, **not** `dist[n]` alone — descending on distance
//! alone reaches the source over a path whose cost is not `dist[dst]`, which is the bug
//! the reference's own comment names.
//!
//! # The tie-break, and why it has to be here
//!
//! On a uniform grid equal-cost paths are the *normal* case, not an edge case: from a
//! source with a clear diagonal, every monotone staircase to the target costs the same.
//! `np.argmin` in the reference returns the first minimum in the stencil order, which is
//! total but accidental — it depends on the row's order rather than naming the rule. Here
//! the minimum is over the pair `(cost, cell)`, so the rule is in the code and in this
//! comment: **on equal cost the lower cell index wins**, always.
//!
//! Nothing iterates a hash map to produce that order. Dijkstra hands back a hash map, and
//! every read here is a point probe at a cell index this module already holds, so no hash
//! order reaches the output — the same discipline Phase 7's `dijkstra_distances` records
//! (`docs/decisions/petgraph-determinism-audit.md`). The map's concrete type is named
//! only by the caller, so nothing in this module depends on which one it is.

use super::csr::{Cell, GridCsr};
use crate::post::grid_index::GridIndex;

/// The distance field Dijkstra solved, as one cost per cell in dense cell order. A cell
/// the field never reached, or one it reached at infinite cost through an obstacle, holds
/// `f64::INFINITY`.
///
/// A slice, not a map: the caller has already converted petgraph's `HashMap` by probing it
/// in dense order (Phase 7's `dijkstra_distances` does the same), so no hash order can
/// reach this module at all — there is none here to depend on.
pub type Field = [f64];

/// The cost of reaching `cell`, or `None` where the field does not hold one.
fn at(field: &Field, cell: Cell) -> Option<f64> {
    field.get(cell as usize).copied()
}

/// A traced route: the cells it crosses, in walk order — destination first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trace {
    /// The cells the walk visited, from where it started to where it ended.
    pub cells: Vec<Cell>,
}

impl Trace {
    /// The same cells, source to destination — the order a route is drawn in.
    pub fn forwards(&self) -> Vec<Cell> {
        let mut cells = self.cells.clone();
        cells.reverse();
        cells
    }
}

/// Walks the field from `start` back toward `source`, or `None` when no route exists.
///
/// An occupied cell other than the two endpoints is never entered. The graph carries every
/// cell — one CSR serves every edge of a run — so this is where the obstacle rule is
/// applied.
pub fn trace(
    graph: &GridCsr,
    grid: &GridIndex,
    field: &Field,
    start: Cell,
    source: Cell,
) -> Option<Trace> {
    let mut cells = vec![start];
    let mut current = start;
    while current != source {
        current = step(graph, grid, field, current, source)?;
        cells.push(current);
    }
    Some(Trace { cells })
}

/// The one cell to move to from `current`, or `None` when the walk cannot descend.
///
/// Two rules, and the order matters — this is the reference's structure
/// (`routed.py::_trace_numpy`), whose two lines are easy to conflate:
///
/// 1. **Pick** the reached, passable neighbour minimising `(dist[n] + w(n, x), n)`. This is
///    not the same as the smallest `dist[n]`: descending on distance alone reaches the
///    source over a path whose cost is not `dist[dst]`, which is the reference's own note.
///    Ties on cost go to the lower cell index, which is this module's total order.
/// 2. **Then guard** on `dist[n] < dist[x]` — the *neighbour's* distance against the
///    *current* cell's, not the step cost against it. On the last step of a route
///    `dist[n] + w(n, x)` equals `dist[x]` exactly, so guarding on the cost would reject
///    every route at its final cell. Guarding on the distance is the reference's `stuck`
///    test: a step that does not descend is a plateau, and the walk would loop.
///
/// `None` rather than a best guess, which is what makes an enclosed node a *reported*
/// fallback instead of a silently wrong route.
fn step(
    graph: &GridCsr,
    grid: &GridIndex,
    field: &Field,
    current: Cell,
    source: Cell,
) -> Option<Cell> {
    let here = at(field, current)?;
    let mut best: Option<(f64, f64, Cell)> = None;
    for (slot, next) in graph.row(current) {
        if grid.is_occupied(next) && next != source {
            continue;
        }
        let Some(there) = at(field, next) else {
            continue;
        };
        let cost = there + graph.cost(slot);
        if !cost.is_finite() {
            continue;
        }
        // `(cost, distance, cell)`: cost picks the step, cell breaks its ties, and the
        // distance rides along so the guard below needs no second pass over the row.
        if best.is_none_or(|(seen, _, cell)| (cost, next) < (seen, cell)) {
            best = Some((cost, there, next));
        }
    }
    best.filter(|&(_, there, _)| there < here)
        .map(|(_, _, cell)| cell)
}

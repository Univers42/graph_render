//! Obstacle-avoiding routing over [`super::grid_index`]'s grid (`prompts/phase-08-post-
//! routing-bundling.md` step 3): the shortest path from one node's cell to another's,
//! treating every other node's cells as obstacles.
//!
//! Reference: `SciGraphs/engine/scigraphs_engine/bundling/routed.py` — Lambert et al.
//! 2010, "winding roads". The reference relaxes a distance field and then traces every
//! edge back from its destination. Both halves are ported, with one deliberate change,
//! recorded below.
//!
//! # What is reused, and what is not
//!
//! The distance field is **Phase 7's Dijkstra**, `petgraph::algo::dijkstra`, run over the
//! grid's own CSR ([`csr`]). This is the reuse the phase demands ("do not write a third
//! shortest-path implementation"): one shortest-path implementation in the crate, the
//! library's, reached through trait impls on our own adjacency exactly as Phase 7 reaches
//! it through `csr_petgraph.rs`.
//!
//! What is **not** reused is the reference's solver. It relaxes by Jacobi sweeps on a
//! regular grid, not by a priority queue, because "Dijkstra wants a priority queue a
//! compute shader cannot have" (`routed.py`, module doc) — a GPU-shaped choice, and this
//! is tier 1a. So `_relax_numpy` and its `eps` convergence test, its `max_rounds` cap and
//! its `float32` step-length table are all absent. What survives is the part that is
//! about the *correctness of the route* rather than the solver: the trace step minimises
//! `dist[n] + w(n, x)`, not `dist[n]` alone ([`trace`]).
//!
//! # Determinism
//!
//! Three places could be non-deterministic, and each is closed:
//!
//! - **Equal-cost paths.** The normal case on a uniform grid, not an edge case: from a
//!   source with a clear diagonal, every monotone staircase to the target costs the same.
//!   [`trace`] breaks them by the explicit total order `(cost, cell index)`, lowest cell
//!   winning.
//! - **The heap.** petgraph's binary heap breaks its own ties arbitrarily, but it returns
//!   *distances*, and the distance to a cell is the minimum over all paths to it — so the
//!   field does not depend on the order cells were popped. Only a *predecessor* would, and
//!   this module never reads one: the predecessor is chosen by [`trace`] from the
//!   finished field. This is Phase 7's own argument, in `analysis/paths.rs`.
//! - **The distance map.** It is a `HashMap`, and nothing iterates it. Every read is a
//!   probe at a cell index the walk already holds.
//!
//! # The flag
//!
//! When no route exists — a node fully enclosed by other nodes' cells — the route is the
//! straight segment and [`Route::straight_fallback`] is `true`. It is a field of the
//! output struct, not a log line: a downstream program cannot read stderr
//! (`prompt.md` §11).

pub mod csr;
#[cfg(test)]
mod measure;
pub mod petgraph_impls;
pub mod trace;

#[cfg(test)]
mod tests;

use crate::columns::EdgeColumns;
use crate::post::grid_index::{GridIndex, GridParams};
use crate::stage::StageError;
use graph_contract::geometry::{NodeGeometry, Paths};
use petgraph::algo::dijkstra;

pub use csr::{GridCsr, build_csr};
pub use petgraph_impls::GridGraph;
pub use trace::Trace;

/// How many cells a route may cross before the walk is called a failure. A shortest path
/// on a grid never revisits a cell, so a walk longer than the cell count has looped; the
/// cap is a backstop against a non-converged field, not a routing policy.
const MAX_STEPS: usize = 4 * 4096;

/// The distance field: one cost per cell, in dense cell order, `f64::INFINITY` where the
/// field never reached. Probed at a cell index, never iterated.
pub type Field = Vec<f64>;

/// One edge's route.
#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    /// The cells it crosses, source to target, both endpoints included. Empty when the
    /// edge's two nodes share a cell and there was nothing to route.
    pub cells: Vec<u32>,
    /// Its vertices in layout coordinates: the source node, each crossed cell's centre,
    /// the target node. Two points — the endpoints — is the straight segment.
    pub points: Vec<(f64, f64)>,
    /// True when the straight segment was used: either no route existed, or the edge's
    /// own nodes share a cell. A caller that needs to tell those apart reads `cells`.
    pub straight_fallback: bool,
}

impl Route {
    /// `true` when the two points are exactly the endpoints, so this row draws straight.
    pub fn is_straight(&self) -> bool {
        self.points.len() == 2
    }
}

/// Every edge's route, in the topology's edge order.
#[derive(Debug, Clone, PartialEq)]
pub struct Routed {
    /// One route per edge, in edge order.
    pub routes: Vec<Route>,
    /// How many edges fell back to the straight segment — the count a caller can act on
    /// without walking the routes.
    pub fallbacks: u32,
}

impl Routed {
    /// Edge `e`'s route.
    pub fn route(&self, e: u32) -> &Route {
        &self.routes[e as usize]
    }

    /// The routes as a polyline column set: edge `e` owns points
    /// `offsets[e]..offsets[e + 1]`, source to target, endpoints included. A two-point
    /// row is the straight segment, which is what the contract draws for an empty
    /// interior.
    pub fn paths(&self) -> Paths {
        let mut offsets = Vec::with_capacity(self.routes.len() + 1);
        let mut pts = Vec::new();
        offsets.push(0);
        for route in &self.routes {
            for (x, y) in &route.points {
                pts.push(*x as f32);
                pts.push(*y as f32);
            }
            offsets.push(u32::try_from(pts.len() / 2).expect("points fit u32"));
        }
        Paths { offsets, pts }
    }
}

/// This module's capability id, the one the hash gate's POST stage list names.
///
/// Not a [`META`](crate::post::META) entry: routing has no `PostRun`-shaped entry point
/// of its own (it needs a grid index and a node geometry, not a handle's snapshot), so
/// graph-core's POST registry does not hold it. It still owns the id — a caller asking for
/// `post.route.grid` is asking for *this* code — so the string lives here, the way
/// `fdeb::ID` and `mingle::ID` do, and graph-wasm's registry row re-exports it rather than
/// spelling it a second time.
pub const ID: &str = "post.route.grid";

/// Routes every edge of `edges` around the nodes in `nodes`, at `params`.
///
/// `edges` supplies only the endpoint columns (`source`, `target`); nothing else about an
/// edge matters to a route, which is what makes this composable with every layout (the
/// phase's step 6 gate).
///
/// O(m · cells · log cells): one Dijkstra per edge over the whole grid. The reference
/// amortises this by solving one field per *distinct source node* and reusing it across
/// that node's edges (`routed.py::_plan`: 168 fields for 592 edges on 240 nodes). Here
/// each edge gets its own field — simpler, and exactly reproducible — at a factor of the
/// average degree. A measured cost, not a hidden one (`docs/measurements/phase08-routing.md`).
pub fn route(
    nodes: &NodeGeometry,
    edges: &EdgeColumns,
    params: &GridParams,
) -> Result<Routed, StageError> {
    let mut grid = GridIndex::new();
    grid.build(nodes, params)?;
    route_over(&mut grid, nodes, edges)
}

/// [`route`], over a grid the caller already built — the form that reuses the buffer
/// across several layouts (`dsa-and-memory.md`: pool what churns).
pub fn route_over(
    grid: &mut GridIndex,
    nodes: &NodeGeometry,
    edges: &EdgeColumns,
) -> Result<Routed, StageError> {
    let graph = build_csr(grid);
    let mut routes = Vec::with_capacity(edges.source.len());
    let mut fallbacks = 0;
    for e in 0..edges.source.len() {
        let route = one_route(grid, &graph, nodes, edges.source[e], edges.target[e]);
        fallbacks += u32::from(route.straight_fallback);
        routes.push(route);
    }
    Ok(Routed { routes, fallbacks })
}

/// One edge's route: the traced cells as a polyline, or the straight segment when the walk
/// fails. The polyline's endpoints are the two nodes' own positions, so the route starts
/// and ends exactly where an edge starts and ends.
fn one_route(grid: &GridIndex, graph: &GridCsr, nodes: &NodeGeometry, a: u32, b: u32) -> Route {
    let from = grid.node_cell(a);
    let to = grid.node_cell(b);
    let straight = || Route {
        cells: Vec::new(),
        points: vec![centre(nodes, a), centre(nodes, b)],
        straight_fallback: true,
    };
    if from == to {
        return straight();
    }
    let field = solve(graph, grid, from, to);
    let Some(walk) = trace::trace(graph, grid, &field, to, from) else {
        return straight();
    };
    if walk.cells.len() > MAX_STEPS {
        return straight();
    }
    let cells = walk.forwards();
    let mut points = Vec::with_capacity(cells.len());
    points.push(centre(nodes, a));
    points.extend(cells[1..cells.len() - 1].iter().map(|c| grid.centre(*c)));
    points.push(centre(nodes, b));
    Route {
        cells,
        points,
        straight_fallback: false,
    }
}

/// Node `n`'s centre, whatever its geometry kind — the only thing a route reads of it,
/// which is why a route composes with every layout that emits a centre.
fn centre(nodes: &NodeGeometry, n: u32) -> (f64, f64) {
    let at = n as usize;
    let (x, y) = match nodes {
        NodeGeometry::Point { x, y } => (x[at], y[at]),
        NodeGeometry::Circle { x, y, .. } => (x[at], y[at]),
        NodeGeometry::Box { x, y, .. } => (x[at], y[at]),
    };
    (f64::from(x), f64::from(y))
}

/// Dijkstra's field from `from` to `to`, with every occupied cell impassable except the
/// two endpoints.
///
/// The obstacle rule is a weight of `f64::INFINITY` rather than a removed edge, so one
/// CSR built from the grid serves every edge of the run. `INFINITY + x` is `INFINITY` and
/// `INFINITY < INFINITY` is false, so an occupied cell is reached at infinite cost, never
/// relaxed down, and the trace's `dist[n] + w(n, x)` minimum ignores it. Dijkstra runs to
/// exhaustion rather than stopping at `to` (`None`), because a route may have to pass
/// *through* a cell whose field is only final later.
/// The distance field Dijkstra solves, as one cost per cell in dense cell order.
///
/// petgraph's `dijkstra` answers in a `HashMap`; this probes it at every cell index
/// `0..cells` in order and drops it, so no hash order survives into the crate and nothing
/// downstream depends on which map petgraph chose. This is Phase 7's own discipline in
/// `analysis::paths.rs::dijkstra_distances`, on the same library.
fn solve(graph: &GridCsr, grid: &GridIndex, from: u32, to: u32) -> Field {
    use petgraph::visit::EdgeRef as _;
    // **Both** endpoints are exempt, and the source matters as much as the target: a node
    // sits in its own cell, so `from` is occupied like any other, and without the exemption
    // every edge *out* of it is infinite and Dijkstra never leaves. That is a real bug this
    // measurement caught — the unit tests missed it because their endpoints were adjacent
    // or the graph was sealed, so the source's own cell never had to be walked out of.
    let scores = dijkstra(GridGraph::new(graph), from, None, |e| {
        if grid.is_occupied(e.target()) && e.target() != to && e.target() != from {
            f64::INFINITY
        } else {
            *e.weight()
        }
    });
    (0..graph.cells())
        .map(|cell| scores.get(&cell).copied().unwrap_or(f64::INFINITY))
        .collect()
}

//! Centrality (`prompts/phase-07-analysis.md` step 4): degree reused verbatim from the
//! topology column, closeness from weighted shortest-path distances (`paths.rs`),
//! betweenness by Brandes' algorithm (petgraph has none), eigenvector by power
//! iteration. Degree, closeness and betweenness are exact — no `Ponytail` owed.
//!
//! **Precondition: every edge weight is non-negative** — the same precondition
//! `paths::dijkstra_distances` documents on itself, and Perron-Frobenius's for
//! [`eigenvector`]. [`betweenness`] needs strictly positive weights (`brandes.rs`). A
//! graph that may carry a negative edge belongs to `paths::bellman_ford` instead;
//! checked with a `debug_assert` in each (same discipline as `components::weak`'s
//! cross-check) so a violation fails loudly in debug/test builds rather than silently
//! returning a wrong number in release.

use crate::analysis::paths::dijkstra_distances;
use crate::csr_petgraph::{CsrDigraph, NodeIx};
use crate::index::Topology;
use petgraph::visit::{EdgeRef as _, IntoEdges as _};

mod brandes;
pub use brandes::betweenness;

/// One analysis id per function this module offers, the way `components::WEAK` is one.
///
/// Four consts rather than one `ID`, because a caller naming
/// `analysis.centrality.betweenness` is asking for *that* function; the string lives
/// beside it, and graph-wasm's registry row and the hash gate's stage list both read it
/// from here instead of spelling it a second time.
pub const DEGREE: &str = "analysis.centrality.degree";

/// [`closeness`]'s analysis id.
pub const CLOSENESS: &str = "analysis.centrality.closeness";

/// [`betweenness`]'s analysis id.
pub const BETWEENNESS: &str = "analysis.centrality.betweenness";

/// [`eigenvector`]'s analysis id.
pub const EIGENVECTOR: &str = "analysis.centrality.eigenvector";

/// Degree, reused verbatim from the column Phase 1 already built (step 4: "already in
/// the topology columns — reuse, do not recompute").
pub fn degree(topology: &Topology) -> &[u32] {
    &topology.nodes().degree
}

/// The module-doc precondition, checked: every edge's `strength` is non-negative. Not a
/// `Ponytail` — this is an exact, deterministic guard, not a heuristic (`ponytail.md`:
/// "not on exact code").
fn non_negative_weights(topology: &Topology) -> bool {
    topology.edges().strength.iter().all(|&w| w >= 0.0)
}

/// Closeness, Wasserman & Faust's disconnected-safe form: the fraction of the graph `v`
/// reaches, scaled by the mean weighted distance to what it reaches. Zero for an
/// isolated node or a graph of one node.
pub fn closeness(topology: &Topology) -> Vec<f32> {
    debug_assert!(
        non_negative_weights(topology),
        "closeness requires non-negative edge weights (module doc precondition); a graph \
         that may carry a negative edge belongs to paths::bellman_ford instead"
    );
    let n = topology.node_count();
    if n < 2 {
        return vec![0.0; n as usize];
    }
    (0..n)
        .map(|v| closeness_of(dijkstra_distances(topology, v), n))
        .collect()
}

/// networkx 3.6 `closeness.py:127-133`: every reached node counts, the source and any
/// peer at distance 0 included (`len(sp) - 1` over `totsp`), scaled by the reached
/// fraction; 0 when the reached distances sum to 0.
///
/// Ponytail: a value past `f32::MAX` (strengths below ~1e-38) is written as `f32::MAX`.
/// Failing input: two nodes 1e-40 apart, true closeness 1e40. Direction: under-reports,
/// and every such node ties at the top. Escape hatch: rescale strengths before ingest.
fn closeness_of(distances: Vec<f64>, n: u32) -> f32 {
    let (reached, sum) = distances
        .iter()
        .filter(|d| d.is_finite())
        .fold((0u32, 0.0), |(count, sum), &d| (count + 1, sum + d));
    let others = reached.saturating_sub(1);
    if others == 0 || sum <= 0.0 {
        return 0.0;
    }
    let fraction = f64::from(others) / f64::from(n - 1);
    (f64::from(others) / sum * fraction).min(f64::from(f32::MAX)) as f32
}

/// Eigenvector centrality by power iteration: a fixed uniform start vector (never the
/// degree column, which would bias toward it), L2-normalised every step, sign-pinned so
/// the largest-magnitude entry is positive — Phase 6's eigenwork reasoning, ported fresh
/// since its `rng.rs`/eigen module is not on this branch's base (a recorded deviation,
/// `docs/measurements/phase07-analysis.md`).
///
/// Ponytail: on a disconnected or bipartite-structured graph the iteration may never
/// settle; past `MAX_ITERS` this returns the last normalised iterate un-converged.
/// Direction: silently plausible-looking, not obviously wrong — a caller ranking nodes
/// across components could be misled. Escape hatch: the returned `bool` is `true` only
/// once the residual is below `TOLERANCE`; do not trust cross-component ranking without
/// it.
pub fn eigenvector(topology: &Topology) -> (Vec<f32>, bool) {
    debug_assert!(
        non_negative_weights(topology),
        "eigenvector centrality requires non-negative edge weights (module doc \
         precondition): a negative weight has no Perron vector to converge to"
    );
    const MAX_ITERS: u32 = 100;
    const TOLERANCE: f64 = 1e-10;
    let n = topology.node_count() as usize;
    if n == 0 {
        return (Vec::new(), true);
    }
    let mut x = vec![1.0 / f64::sqrt(n as f64); n];
    for _ in 0..MAX_ITERS {
        let mut next = vec![0.0; n];
        for v in 0..n as u32 {
            let graph = CsrDigraph::new(topology);
            for edge in graph.edges(NodeIx(v)) {
                next[edge.target().0 as usize] += x[v as usize] * edge.weight();
            }
        }
        let Some(residual) = normalize_and_pin(&mut next, &x) else {
            return (vec![0.0; n], false);
        };
        x = next;
        if residual < TOLERANCE {
            return (x.into_iter().map(|v| v as f32).collect(), true);
        }
    }
    (x.into_iter().map(|v| v as f32).collect(), false)
}

/// L2-normalises `next`, pins its sign so the largest-magnitude entry is positive, and
/// returns the max change from `previous` — `None` if `next` collapsed to zero or its
/// norm is not finite.
///
/// Ponytail: an iterate whose norm overflows `f64` (strengths near `f64::MAX`) ends the
/// run as `(zeros, false)` rather than NaN (D9), though the true vector exists. Failing
/// input: two parallel edges of strength 1.7e308. Direction: refuses, never misleads.
/// Escape hatch: rescale strengths before ingest; the vector is scale-invariant.
fn normalize_and_pin(next: &mut [f64], previous: &[f64]) -> Option<f64> {
    let norm = f64::sqrt(next.iter().map(|v| v * v).sum::<f64>());
    if norm == 0.0 || !norm.is_finite() {
        return None;
    }
    next.iter_mut().for_each(|v| *v /= norm);
    let dominant = next
        .iter()
        .copied()
        .reduce(|a, b| if b.abs() > a.abs() { b } else { a });
    if dominant.unwrap_or(0.0) < 0.0 {
        next.iter_mut().for_each(|v| *v = -*v);
    }
    Some(
        previous
            .iter()
            .zip(next.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f64::max),
    )
}

// House limit: <=300 lines per file. Split the same way `csr_petgraph.rs` is
// (`csr_petgraph/tests.rs`), rather than let a growing test module push this file over.
#[cfg(test)]
mod tests;

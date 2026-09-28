//! Centrality (`prompts/phase-07-analysis.md` step 4): degree reused verbatim from the
//! topology column, closeness from weighted shortest-path distances (`paths.rs`),
//! betweenness by Brandes' algorithm (petgraph has none), eigenvector by power
//! iteration. Degree, closeness and betweenness are exact — no `Ponytail` owed.

use crate::analysis::paths::dijkstra_distances;
use crate::csr_petgraph::{CsrDigraph, NodeIx};
use crate::index::Topology;
use petgraph::visit::{EdgeRef as _, IntoEdges as _};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Degree, reused verbatim from the column Phase 1 already built (step 4: "already in
/// the topology columns — reuse, do not recompute").
pub fn degree(topology: &Topology) -> &[u32] {
    &topology.nodes().degree
}

/// Closeness, Wasserman & Faust's disconnected-safe form: the fraction of the graph `v`
/// reaches, scaled by the mean weighted distance to what it reaches. Zero for an
/// isolated node or a graph of one node.
pub fn closeness(topology: &Topology) -> Vec<f32> {
    let n = topology.node_count();
    if n < 2 {
        return vec![0.0; n as usize];
    }
    (0..n)
        .map(|v| closeness_of(dijkstra_distances(topology, v), n))
        .collect()
}

fn closeness_of(distances: Vec<f64>, n: u32) -> f32 {
    let (reachable, sum) = distances
        .iter()
        .filter(|&&d| d.is_finite() && d > 0.0)
        .fold((0u32, 0.0), |(count, sum), &d| (count + 1, sum + d));
    if reachable == 0 || sum <= 0.0 {
        return 0.0;
    }
    let fraction = f64::from(reachable) / f64::from(n - 1);
    (f64::from(reachable) / sum * fraction) as f32
}

/// A `(distance, node)` pair ordered so [`BinaryHeap`] (a max-heap) pops the smallest
/// distance first, ties broken by the smaller dense index (D5) — the same discipline
/// `paths::dijkstra_distances` inherits from petgraph's own heap.
struct MinScore(f64, u32);

impl PartialEq for MinScore {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for MinScore {}
impl PartialOrd for MinScore {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for MinScore {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .0
            .partial_cmp(&self.0)
            .unwrap_or(Ordering::Equal)
            .then_with(|| other.1.cmp(&self.1))
    }
}

/// Betweenness centrality (Brandes 2001): for every ordered pair `(s, t)`, the share of
/// shortest `s`→`t` paths through `v`, summed over every pair. O(nm) exact — the only
/// tractable exact method; not tried past its declared ceiling
/// (`docs/measurements/phase07-analysis.md`). Never silently sampled: a sampled variant,
/// if it ever ships, is a separately named capability (step 4, stop-and-ask).
pub fn betweenness(topology: &Topology) -> Vec<f32> {
    let n = topology.node_count() as usize;
    let mut score = vec![0.0f64; n];
    for s in 0..n as u32 {
        accumulate_from(topology, s, &mut score);
    }
    score.into_iter().map(|v| v as f32).collect()
}

/// One source's contribution: Dijkstra that also counts shortest paths (`sigma`) and
/// keeps every tied predecessor, then Brandes' back-accumulation over the finish order.
fn accumulate_from(topology: &Topology, s: u32, score: &mut [f64]) {
    let n = topology.node_count() as usize;
    let (order, sigma, preds) = shortest_path_dag(topology, s, n);
    let mut delta = vec![0.0f64; n];
    for &w in order.iter().rev() {
        for &v in &preds[w as usize] {
            delta[v as usize] += sigma[v as usize] / sigma[w as usize] * (1.0 + delta[w as usize]);
        }
        if w != s {
            score[w as usize] += delta[w as usize];
        }
    }
}

/// Dijkstra from `s`, additionally tracking the path count and every tied predecessor
/// of each node — Brandes' shortest-path DAG, not exposed by petgraph's own `dijkstra`.
#[allow(clippy::type_complexity)]
fn shortest_path_dag(topology: &Topology, s: u32, n: usize) -> (Vec<u32>, Vec<f64>, Vec<Vec<u32>>) {
    let graph = CsrDigraph::new(topology);
    let (mut dist, mut sigma) = (vec![f64::INFINITY; n], vec![0.0f64; n]);
    let mut preds: Vec<Vec<u32>> = vec![Vec::new(); n];
    let (mut order, mut done) = (Vec::with_capacity(n), vec![false; n]);
    (dist[s as usize], sigma[s as usize]) = (0.0, 1.0);
    let mut heap = BinaryHeap::from([MinScore(0.0, s)]);
    while let Some(MinScore(d, v)) = heap.pop() {
        if done[v as usize] {
            continue;
        }
        done[v as usize] = true;
        order.push(v);
        for edge in graph.edges(NodeIx(v)) {
            let w = edge.target().0;
            if done[w as usize] {
                continue;
            }
            let candidate = d + edge.weight();
            match candidate.partial_cmp(&dist[w as usize]) {
                Some(Ordering::Less) => {
                    (dist[w as usize], sigma[w as usize]) = (candidate, sigma[v as usize]);
                    preds[w as usize] = vec![v];
                    heap.push(MinScore(candidate, w));
                }
                Some(Ordering::Equal) => {
                    sigma[w as usize] += sigma[v as usize];
                    preds[w as usize].push(v);
                }
                _ => {}
            }
        }
    }
    (order, sigma, preds)
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
    const MAX_ITERS: u32 = 100;
    const TOLERANCE: f64 = 1e-10;
    let n = topology.node_count() as usize;
    if n == 0 {
        return (Vec::new(), true);
    }
    let mut x = vec![1.0 / libm::sqrt(n as f64); n];
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
/// returns the max change from `previous` — `None` if `next` collapsed to zero.
fn normalize_and_pin(next: &mut [f64], previous: &[f64]) -> Option<f64> {
    let norm = libm::sqrt(next.iter().map(|v| v * v).sum::<f64>());
    if norm == 0.0 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    /// A 4-point star: `center` is the sole cut vertex on every leaf-leaf path.
    fn star(leaves: u32) -> Topology {
        let mut nodes = vec![node("center", "")];
        let mut edges = Vec::new();
        for i in 0..leaves {
            let leaf = format!("leaf{i}");
            nodes.push(node(&leaf, ""));
            edges.push(edge(&format!("e{i}"), "center", &leaf));
        }
        index_model(&nodes, &edges).expect("fits")
    }

    #[test]
    fn degree_is_the_topology_column_not_a_recomputation() {
        let t = star(3);
        assert_eq!(degree(&t), t.nodes().degree.as_slice());
        assert_eq!(degree(&t)[0], 3, "the center touches every leaf");
    }

    #[test]
    fn on_a_star_only_the_center_has_positive_betweenness() {
        let t = star(4);
        let b = betweenness(&t);
        assert!(
            b[0] > 0.0,
            "every leaf pair's shortest path crosses the center"
        );
        assert!(
            b[1..].iter().all(|&v| v == 0.0),
            "leaves sit on no one else's path"
        );
    }

    #[test]
    fn the_center_of_a_star_is_closer_to_everyone_than_any_leaf() {
        let t = star(3);
        let c = closeness(&t);
        assert!(c[0] > c[1], "center: {}, leaf: {}", c[0], c[1]);
    }

    /// hub-x, hub-y, x-y (a triangle: an odd cycle, so this graph is *not* bipartite)
    /// plus hub-leaf. Power iteration on a bipartite graph oscillates between the two
    /// eigenvectors of `+lambda_max` and `-lambda_max` forever (equal magnitude, so
    /// neither dominates) rather than converging — [`star`] below is exactly that case,
    /// which is why convergence is checked against this graph instead.
    fn triangle_with_pendant() -> Topology {
        let nodes = [
            node("hub", ""),
            node("x", ""),
            node("y", ""),
            node("leaf", ""),
        ];
        let edges = [
            edge("hx", "hub", "x"),
            edge("hy", "hub", "y"),
            edge("xy", "x", "y"),
            edge("hl", "hub", "leaf"),
        ];
        index_model(&nodes, &edges).expect("fits")
    }

    #[test]
    fn eigenvector_converges_and_favours_the_higher_degree_node() {
        let (scores, converged) = eigenvector(&triangle_with_pendant());
        assert!(converged);
        assert!(scores[0] > scores[3], "the hub outranks the pendant leaf");
        assert!(scores[0] > 0.0);
    }

    #[test]
    fn eigenvector_on_a_bipartite_star_reports_non_convergence_not_a_plausible_lie() {
        // Ponytail (module doc): a bipartite graph's two extremal eigenvalues tie in
        // magnitude, so plain power iteration never settles. The star is bipartite
        // (center vs. leaves) by construction — this must come back `false`, not a
        // number that merely looks like an answer.
        let (_, converged) = eigenvector(&star(3));
        assert!(
            !converged,
            "a star is bipartite: this must not silently claim convergence"
        );
    }

    #[test]
    fn repeated_runs_agree_bit_for_bit() {
        let t = star(3);
        assert_eq!(closeness(&t), closeness(&t));
        assert_eq!(betweenness(&t), betweenness(&t));
        assert_eq!(eigenvector(&t), eigenvector(&t));
    }
}

//! Shortest paths (`prompts/phase-07-analysis.md` step 3): Dijkstra for non-negative
//! weights, reused from petgraph; Bellman-Ford for graphs that may carry a negative
//! edge, including the negative-cycle report SciGraphs never gave its operator
//! (`prompt.md` §11 guardrail 8; `prompts/REFERENCES.md` Tier 4). Exact — no `Ponytail`
//! owed on either.
//!
//! Weight is `edge.strength`, the only per-edge scalar the topology carries; a fixture
//! that needs a specific weight sets it directly (`fixtures/analysis/*.json`).

use crate::csr_petgraph::{CsrDigraph, NodeIx};
use crate::index::Topology;
use petgraph::visit::EdgeRef as _;

/// Dijkstra's shortest distances from `source`, `f64::INFINITY` where unreached.
/// **Precondition: every reachable edge weight is non-negative** — a graph that may
/// carry a negative edge belongs to [`bellman_ford`] instead. A `source` past the last
/// node panics by name.
///
/// petgraph's `dijkstra` returns a `HashMap`; this probes it at every dense index
/// `0..n` in order rather than iterating it, so no hash order reaches the output
/// (`docs/decisions/petgraph-determinism-audit.md`). The *distances* Dijkstra returns
/// do not depend on how its binary heap breaks ties among equal-cost frontier entries —
/// only a predecessor/path would, and this function returns distances only.
pub fn dijkstra_distances(topology: &Topology, source: u32) -> Vec<f64> {
    check_source(topology, source);
    let graph = CsrDigraph::new(topology);
    let scores = petgraph::algo::dijkstra(graph, NodeIx(source), None, |e| *e.weight());
    (0..topology.node_count())
        .map(|v| scores.get(&NodeIx(v)).copied().unwrap_or(f64::INFINITY))
        .collect()
}

/// What [`bellman_ford`] found.
#[derive(Debug, Clone, PartialEq)]
pub enum ShortestPaths {
    /// One distance per node, `f64::INFINITY` where unreached. Indexed by dense index —
    /// petgraph's own `Vec`, not a `HashMap`, so nothing here needed the audit above.
    Distances(Vec<f64>),
    /// `source` can reach a negative-weight cycle: its nodes, in cycle order, the
    /// answer this phase exists to give instead of a silently wrong distance.
    NegativeCycle(Vec<u32>),
}

/// Bellman-Ford from `source`. Negative edges are permitted; a negative cycle reachable
/// from `source` is reported by name, never folded into a distance that would be
/// nonsense. A `source` past the last node panics by name.
pub fn bellman_ford(topology: &Topology, source: u32) -> ShortestPaths {
    check_source(topology, source);
    let graph = CsrDigraph::new(topology);
    match petgraph::algo::bellman_ford(graph, NodeIx(source)) {
        Ok(paths) => ShortestPaths::Distances(paths.distances),
        Err(_) => {
            let cycle = petgraph::algo::find_negative_cycle(graph, NodeIx(source))
                .expect("bellman_ford found a negative cycle; find_negative_cycle must too");
            ShortestPaths::NegativeCycle(cycle.into_iter().map(|NodeIx(v)| v).collect())
        }
    }
}

/// A source `>= node_count` is a caller bug, refused by name before petgraph indexes it.
fn check_source(topology: &Topology, source: u32) {
    let n = topology.node_count();
    assert!(source < n, "source {source} of {n}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    fn directed(id: &str, a: &str, b: &str, weight: f64) -> crate::records::EdgeRecord {
        let mut e = edge(id, a, b);
        e.directed = true;
        e.strength = weight;
        e
    }

    /// The counter-example the phase exists to prove: A→B(2), A→C(3), C→B(−2). The true
    /// shortest A→B is 1 (via C), but Dijkstra finalises B at 2 before it ever relaxes
    /// C→B, so it is a *different, wrong* answer — this is the phase's proof, not a
    /// hypothetical (`prompts/phase-07-analysis.md` step 3).
    #[test]
    fn bellman_ford_negative_weight_gives_a_different_correct_answer_from_dijkstra() {
        let nodes = [node("a", ""), node("b", ""), node("c", "")];
        let edges = [
            directed("ab", "a", "b", 2.0),
            directed("ac", "a", "c", 3.0),
            directed("cb", "c", "b", -2.0),
        ];
        let t = index_model(&nodes, &edges).expect("fits");
        assert_eq!(
            dijkstra_distances(&t, 0),
            [0.0, 2.0, 3.0],
            "Dijkstra: wrong, B finalised before the negative edge relaxes it"
        );
        assert_eq!(
            bellman_ford(&t, 0),
            ShortestPaths::Distances(vec![0.0, 1.0, 3.0]),
            "Bellman-Ford: correct — A->C->B costs 1"
        );
    }

    #[test]
    fn negative_cycle_detected_names_the_cycle_not_a_bogus_distance() {
        let nodes = [node("a", ""), node("b", ""), node("c", "")];
        let edges = [
            directed("ab", "a", "b", 1.0),
            directed("bc", "b", "c", -3.0),
            directed("ca", "c", "a", 1.0),
        ];
        let t = index_model(&nodes, &edges).expect("fits");
        let ShortestPaths::NegativeCycle(mut cycle) = bellman_ford(&t, 0) else {
            panic!("a->b->c->a sums to -1: this must be reported as a cycle");
        };
        cycle.sort_unstable();
        assert_eq!(
            cycle,
            [0, 1, 2],
            "every node on the cycle, no duplicate, no ghost"
        );
    }

    #[test]
    fn an_unreachable_node_is_infinite_in_both_algorithms() {
        let nodes = [node("a", ""), node("b", ""), node("z", "")];
        let t = index_model(&nodes, &[directed("ab", "a", "b", 1.0)]).expect("fits");
        assert_eq!(dijkstra_distances(&t, 0)[2], f64::INFINITY);
        assert_eq!(
            bellman_ford(&t, 0),
            ShortestPaths::Distances(vec![0.0, 1.0, f64::INFINITY])
        );
    }

    #[test]
    fn repeated_runs_agree_bit_for_bit() {
        let nodes = [node("a", ""), node("b", ""), node("c", "")];
        let edges = [directed("ab", "a", "b", 1.0), directed("bc", "b", "c", 4.0)];
        let t = index_model(&nodes, &edges).expect("fits");
        assert_eq!(dijkstra_distances(&t, 0), dijkstra_distances(&t, 0));
        assert_eq!(bellman_ford(&t, 0), bellman_ford(&t, 0));
    }

    /// M27 (`docs/reviews/review-core-post.md`): a source past the last node is a caller
    /// bug refused by name, as `depth::depth_from` refuses a root, not a slice index.
    #[test]
    #[should_panic(expected = "source 0 of 0")]
    fn dijkstra_refuses_a_source_past_the_last_node_by_name() {
        dijkstra_distances(&index_model(&[], &[]).expect("fits"), 0);
    }

    #[test]
    #[should_panic(expected = "source 2 of 2")]
    fn bellman_ford_refuses_a_source_past_the_last_node_by_name() {
        let t = index_model(&[node("a", ""), node("b", "")], &[]).expect("fits");
        bellman_ford(&t, 2);
    }
}

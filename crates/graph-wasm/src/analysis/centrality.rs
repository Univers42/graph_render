//! Centrality analyses: degree, closeness, betweenness, eigenvector.

use super::report::{Column, Report};
use graph_core::Topology;
use graph_core::analysis::centrality;

/// Degree centrality analysis entry point.
pub fn degree_centrality(topology: &Topology) -> Report {
    let values = centrality::degree(topology)
        .iter()
        .map(|&d| f64::from(d))
        .collect();
    plain(centrality::DEGREE, values)
}

/// Closeness centrality analysis entry point.
pub fn closeness_centrality(topology: &Topology) -> Report {
    plain(
        centrality::CLOSENESS,
        shortest_path_scores(topology, centrality::closeness),
    )
}

/// Betweenness centrality analysis entry point.
pub fn betweenness_centrality(topology: &Topology) -> Report {
    plain(
        centrality::BETWEENNESS,
        shortest_path_scores(topology, centrality::betweenness),
    )
}

/// A Dijkstra-based centrality, or a column of `NaN` when any edge `strength` is negative.
/// Ingest admits any finite strength (F-80), and over a negative one graph-core's Dijkstra
/// asserts in a debug build and answers silently wrong in a release one. `NaN` is the
/// honest score (no shortest path is defined), and it is also what makes
/// [`Report::to_json`] answer `None`, so `gm_analysis_run` refuses with `AnalysisFailed`.
fn shortest_path_scores(topology: &Topology, scores: fn(&Topology) -> Vec<f32>) -> Vec<f64> {
    if has_negative_strength(topology) {
        return refused(topology);
    }
    widened(scores(topology))
}

/// Eigenvector centrality analysis entry point.
/// A negative `strength` has no Perron vector, and graph-core's eigenvector asserts on
/// one in a debug build (R19); it is refused the way the Dijkstra centralities are.
pub fn eigenvector_centrality(topology: &Topology) -> Report {
    let (values, converged) = if has_negative_strength(topology) {
        (refused(topology), false)
    } else {
        let (values, converged) = centrality::eigenvector(topology);
        (widened(values), converged)
    };
    Report {
        id: centrality::EIGENVECTOR,
        values: Column::F64(values),
        converged: Some(converged),
        modularity: None,
        max: None,
    }
}

fn has_negative_strength(topology: &Topology) -> bool {
    topology
        .edges()
        .strength
        .iter()
        .any(|&strength| strength < 0.0)
}

/// The column that makes [`Report::to_json`] answer `None`: one `NaN` per node.
fn refused(topology: &Topology) -> Vec<f64> {
    vec![f64::NAN; topology.node_count() as usize]
}

/// `f32` as graph-core states a centrality, widened to the `f64` the wire carries. Exact:
/// every `f32` is a `f64`, so this loses nothing and changes no comparison.
fn widened(values: Vec<f32>) -> Vec<f64> {
    values.into_iter().map(f64::from).collect()
}

fn plain(id: &'static str, values: Vec<f64>) -> Report {
    Report {
        id,
        values: Column::F64(values),
        converged: None,
        modularity: None,
        max: None,
    }
}

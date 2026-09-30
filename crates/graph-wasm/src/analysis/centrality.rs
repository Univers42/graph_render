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
        widened(centrality::closeness(topology)),
    )
}

/// Betweenness centrality analysis entry point.
pub fn betweenness_centrality(topology: &Topology) -> Report {
    plain(
        centrality::BETWEENNESS,
        widened(centrality::betweenness(topology)),
    )
}

/// Eigenvector centrality analysis entry point.
pub fn eigenvector_centrality(topology: &Topology) -> Report {
    let (values, converged) = centrality::eigenvector(topology);
    Report {
        id: centrality::EIGENVECTOR,
        values: Column::F64(widened(values)),
        converged: Some(converged),
        modularity: None,
        max: None,
    }
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

//! Louvain communities analysis.

use super::report::{Column, Report};
use graph_core::Topology;
use graph_core::analysis::communities;

/// Louvain communities analysis entry point.
pub fn louvain_communities(topology: &Topology) -> Report {
    let labels = communities::louvain(topology);
    let modularity = communities::modularity(topology, &labels);
    Report {
        id: communities::LOUVAIN,
        values: Column::U32(labels),
        converged: None,
        modularity: Some(modularity),
        max: None,
    }
}
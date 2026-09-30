//! Weak and strong components analyses.

use super::report::{Column, Report};
use graph_core::Topology;
use graph_core::analysis::components;

/// Weak components analysis entry point.
pub fn weak_components(topology: &Topology) -> Report {
    Report {
        id: components::WEAK,
        values: Column::U32(components::weak(topology)),
        converged: None,
        modularity: None,
        max: None,
    }
}

/// Strong components analysis entry point.
pub fn strong_components(topology: &Topology) -> Report {
    Report {
        id: components::STRONG,
        values: Column::U32(components::strong(topology)),
        converged: None,
        modularity: None,
        max: None,
    }
}

//! The analysis registry: the table of analyses the ABI exposes, and the functions to
//! query and run them.

use crate::analysis::{components, communities, centrality, depth};
use super::report::{Entry, Report};
use graph_core::Topology;

/// Every analysis the ABI exposes, in registration order. Append-only: a row added here
/// is discoverable through `gm_analysis_count`/`gm_analysis_id` with no ABI change, the
/// property C1 states for layouts.
///
/// **Every id is graph-core's own constant**, not a spelling here — `analysis::*::ID`
/// from the module that computes the analysis, the way `post::*::ID` is for the POST
/// rows below. One id in one place means a row cannot be filed under one string while its
/// `Report` announces another, and the hash gate's stage list reads the same constants.
pub static ANALYSES: [Entry; 8] = [
    Entry {
        id: graph_core::analysis::components::WEAK,
        run: components::weak_components,
    },
    Entry {
        id: graph_core::analysis::components::STRONG,
        run: components::strong_components,
    },
    Entry {
        id: graph_core::analysis::communities::LOUVAIN,
        run: communities::louvain_communities,
    },
    Entry {
        id: graph_core::analysis::centrality::DEGREE,
        run: centrality::degree_centrality,
    },
    Entry {
        id: graph_core::analysis::centrality::CLOSENESS,
        run: centrality::closeness_centrality,
    },
    Entry {
        id: graph_core::analysis::centrality::BETWEENNESS,
        run: centrality::betweenness_centrality,
    },
    Entry {
        id: graph_core::analysis::centrality::EIGENVECTOR,
        run: centrality::eigenvector_centrality,
    },
    Entry {
        id: graph_core::analysis::depth::BFS,
        run: depth::bfs_depth,
    },
];

/// How many analyses the ABI exposes; `gm_analysis_count`'s body.
pub fn count() -> u32 {
    u32::try_from(ANALYSES.len()).unwrap_or(0)
}

/// The id at index `i`, or `None` past the end — the same shape `gm_layout_id` and
/// `gm_post_id` answer out of range with.
pub fn id_at(i: u32) -> Option<&'static str> {
    ANALYSES.get(i as usize).map(|entry| entry.id)
}

/// The analysis at index `i` run over `topology`, or `None` past the end. The one body
/// `gm_analysis_run` delegates to, so both of its refusals are pinned natively.
pub fn run(i: u32, topology: &Topology) -> Option<Report> {
    ANALYSES.get(i as usize).map(|entry| (entry.run)(topology))
}

/// The JSON face of the analysis at index `i` over `topology`: canonical, ascending-key
/// order, one line, no trailing newline. `None` past the end of the registry.
pub fn to_json(i: u32, topology: &Topology) -> Option<String> {
    let report = run(i, topology)?;
    Some(report.to_json())
}
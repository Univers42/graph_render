//! The eight registry rows: each one calls graph-core's function as it is and shapes a `Report`.

use super::{Column, Report};
use graph_core::Topology;
use graph_core::analysis::depth;
use graph_core::analysis::{centrality, communities, components};
use graph_core::layout::hierarchy::Hierarchy;

pub(super) fn weak_components(topology: &Topology) -> Report {
    labelled("analysis.components.weak", components::weak(topology))
}

pub(super) fn strong_components(topology: &Topology) -> Report {
    labelled("analysis.components.strong", components::strong(topology))
}

pub(super) fn louvain_communities(topology: &Topology) -> Report {
    let labels = communities::louvain(topology);
    let modularity = communities::modularity(topology, &labels);
    Report {
        id: "analysis.communities.louvain",
        values: Column::U32(labels),
        converged: None,
        modularity: Some(modularity),
        max: None,
    }
}

fn labelled(id: &'static str, labels: Vec<u32>) -> Report {
    Report {
        id,
        values: Column::U32(labels),
        converged: None,
        modularity: None,
        max: None,
    }
}

pub(super) fn degree_centrality(topology: &Topology) -> Report {
    let values = centrality::degree(topology)
        .iter()
        .map(|&d| f64::from(d))
        .collect();
    plain("analysis.centrality.degree", values)
}

pub(super) fn closeness_centrality(topology: &Topology) -> Report {
    plain(
        "analysis.centrality.closeness",
        widened(centrality::closeness(topology)),
    )
}

pub(super) fn betweenness_centrality(topology: &Topology) -> Report {
    plain(
        "analysis.centrality.betweenness",
        widened(centrality::betweenness(topology)),
    )
}

pub(super) fn eigenvector_centrality(topology: &Topology) -> Report {
    let (values, converged) = centrality::eigenvector(topology);
    Report {
        id: "analysis.centrality.eigenvector",
        values: Column::F64(widened(values)),
        converged: Some(converged),
        modularity: None,
        max: None,
    }
}

/// `f32` as graph-core states a centrality, widened to the `f64` the wire carries. Exact:
/// every `f32` is a `f64`, so this loses nothing and changes no comparison.
pub(super) fn widened(values: Vec<f32>) -> Vec<f64> {
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

/// BFS depth over the repaired hierarchy — graph-core's own `Hierarchy`, which
/// graph-core's `analysis::depth` reads directly, so this face and graph-core's own
/// depth column are one convention and not two (see [`depth::Roots`]).
pub(super) fn bfs_depth(topology: &Topology) -> Report {
    let depth = forest_depth(topology);
    Report {
        id: "analysis.depth.bfs",
        values: Column::U32(depth.levels().to_vec()),
        converged: None,
        modularity: None,
        max: Some(depth.max()),
    }
}

/// The depth of every node of `topology` under the hierarchy convention, including the
/// virtual root's two-plus-roots case, which graph-core's own `Hierarchy` already
/// encodes — so this reads one convention, not two.
///
/// `Hierarchy::of`'s only refusal is `n + 1` not fitting `u32`, i.e. a graph with
/// `u32::MAX` nodes, which `index_model` cannot have produced (its own capacity check
/// runs first). So this is a caller-bug panic, the same discipline `Csr::from_pairs`
/// takes for an out-of-range row, and not a path a real handle reaches.
fn forest_depth(topology: &Topology) -> depth::Depth {
    let hierarchy = Hierarchy::of(topology).expect("n + 1 fits u32 for any indexed topology");
    depth::bfs_depth(&hierarchy)
}

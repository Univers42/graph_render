//! BFS depth analysis.

use super::report::{Column, Report};
use graph_core::Topology;
use graph_core::analysis::depth;
use graph_core::layout::hierarchy::Hierarchy;

/// BFS depth analysis entry point.
pub fn bfs_depth(topology: &Topology) -> Report {
    let depth = forest_depth(topology);
    Report {
        id: depth::BFS,
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
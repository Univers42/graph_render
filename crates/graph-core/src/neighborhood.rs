//! Breadth-first neighbourhood (`src/core/model/neighborhood.ts`): a node plus
//! everything within `depth` hops, and the edges walked to reach them. O(reachable).

use crate::arena::FixedState;
use crate::index::Topology;
use indexmap::IndexSet;

/// Nodes and edges reached, each in the order the oracle's `Set`s receive them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Neighborhood {
    /// Dense node indices, the start node first.
    pub nodes: Vec<u32>,
    /// Dense edge indices, every edge incident to a node on a walked frontier.
    pub edges: Vec<u32>,
}

/// `neighborhood` (`neighborhood.ts:52-54`): the node ids only.
pub fn neighborhood(topology: &Topology, id: &str, depth: u32) -> Vec<u32> {
    neighborhood_edges(topology, id, depth).nodes
}

/// `neighborhoodEdges` (`neighborhood.ts:61-67`), the shared BFS (`:19-49`). An id not
/// in the topology yields an empty result, never a set holding the phantom id.
///
/// `depth` is a hop count. The oracle takes any JS number and walks `ceil(depth)` hops
/// for a positive one and none for `NaN` or `<= 0`; a caller with such a value converts
/// it first. Its default, 1, is the caller's to pass.
pub fn neighborhood_edges(topology: &Topology, id: &str, depth: u32) -> Neighborhood {
    let Some(start) = topology.node_index(id) else {
        return Neighborhood::default();
    };
    let mut nodes = IndexSet::<u32, FixedState>::default();
    let mut edges = IndexSet::<u32, FixedState>::default();
    nodes.insert(start);
    let mut frontier = vec![start];
    let columns = topology.edges();
    for _ in 0..depth {
        let mut next = Vec::new();
        for &current in &frontier {
            for edge in topology.incident(current) {
                edges.insert(edge);
                let (source, target) =
                    (columns.source[edge as usize], columns.target[edge as usize]);
                let other = if source == current { target } else { source };
                if nodes.insert(other) {
                    next.push(other);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    Neighborhood {
        nodes: nodes.into_iter().collect(),
        edges: edges.into_iter().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::records::build::{edge, node};

    /// a - b - c - d, plus a self-loop on b and a second a-b edge.
    fn chain() -> Topology {
        let nodes = ["a", "b", "c", "d", "lone"].map(|id| node(id, ""));
        let edges = [
            edge("ab", "a", "b"),
            edge("bb", "b", "b"),
            edge("bc", "c", "b"),
            edge("cd", "c", "d"),
            edge("ab2", "b", "a"),
        ];
        index_model(&nodes, &edges).expect("fits")
    }

    #[test]
    fn one_hop_takes_every_incident_edge_and_its_far_ends() {
        let t = chain();
        let hood = neighborhood_edges(&t, "b", 1);
        assert_eq!(hood.nodes, [1, 0, 2]);
        assert_eq!(hood.edges, [0, 1, 2, 4]);
        assert_eq!(neighborhood(&t, "b", 1), hood.nodes);
    }

    #[test]
    fn two_hops_walk_the_frontier_in_discovery_order() {
        let hood = neighborhood_edges(&chain(), "a", 2);
        assert_eq!(hood.nodes, [0, 1, 2]);
        assert_eq!(hood.edges, [0, 4, 1, 2]);
    }

    #[test]
    fn depth_zero_is_the_node_alone_and_a_huge_depth_stops_when_nothing_is_new() {
        let t = chain();
        let alone = neighborhood_edges(&t, "c", 0);
        assert_eq!((alone.nodes, alone.edges), (vec![2], vec![]));
        let all = neighborhood_edges(&t, "d", u32::MAX);
        assert_eq!(all.nodes, [3, 2, 1, 0]);
        assert_eq!(all.edges, [3, 2, 0, 1, 4]);
        assert_eq!(neighborhood(&t, "lone", 3), [4]);
    }

    #[test]
    fn an_unknown_id_is_empty_not_a_phantom() {
        assert_eq!(
            neighborhood_edges(&chain(), "gone", 2),
            Neighborhood::default()
        );
    }
}

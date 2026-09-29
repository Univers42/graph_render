//! One coarsening step: a maximal matching, heaviest edge first, lowest index on ties.
//! Matched pairs collapse into one coarse node; parallel coarse edges keep the first
//! one's strength (the same convention `simple_graph` states).

use crate::arena::FixedState;
use crate::layout::force::SimpleGraph;
use indexmap::IndexMap;

/// A coarse graph and the fine-to-coarse node map that produced it.
pub(super) struct Coarse {
    pub(super) graph: SimpleGraph,
    pub(super) node_count: u32,
    pub(super) map: Vec<u32>,
}

/// Ponytail: greedy matching in ascending index order is not a maximum matching, and it
/// depends on the dense index, so a relabelled graph coarsens differently (the layout
/// stays deterministic for a given topology, never label-invariant). A star matches one
/// leaf per level, so it coarsens by one node and the caller stops on the stall guard.
pub(super) fn coarsen(g: &SimpleGraph, n: u32) -> Coarse {
    let mut map = vec![u32::MAX; n as usize];
    let mut next = 0u32;
    for v in 0..n {
        if map[v as usize] != u32::MAX {
            continue;
        }
        map[v as usize] = next;
        if let Some(u) = best_free_neighbour(g, v, &map) {
            map[u as usize] = next;
        }
        next += 1;
    }
    Coarse {
        graph: quotient(g, &map, next),
        node_count: next,
        map,
    }
}

fn best_free_neighbour(g: &SimpleGraph, v: u32, map: &[u32]) -> Option<u32> {
    let mut best: Option<(f64, u32)> = None;
    for &e in g.rows.row(v) {
        let e = e as usize;
        let u = g.lo[e] ^ g.hi[e] ^ v;
        if map[u as usize] != u32::MAX {
            continue;
        }
        let better = match best {
            None => true,
            Some((s, w)) => g.strength[e] > s || (g.strength[e] == s && u < w),
        };
        if better {
            best = Some((g.strength[e], u));
        }
    }
    best.map(|(_, u)| u)
}

fn quotient(g: &SimpleGraph, map: &[u32], count: u32) -> SimpleGraph {
    let mut seen: IndexMap<(u32, u32), (), FixedState> = IndexMap::default();
    let (mut lo, mut hi, mut strength) = (Vec::new(), Vec::new(), Vec::new());
    for e in 0..g.lo.len() {
        let (a, b) = (map[g.lo[e] as usize], map[g.hi[e] as usize]);
        if a == b || seen.insert((a.min(b), a.max(b)), ()).is_some() {
            continue;
        }
        lo.push(a.min(b));
        hi.push(a.max(b));
        strength.push(g.strength[e]);
    }
    SimpleGraph::from_edges(count, lo, hi, strength)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::index_model;
    use crate::layout::force::simple_graph;
    use crate::records::build::{edge, node};

    fn path(n: usize) -> SimpleGraph {
        let names: Vec<String> = (0..n).map(|i| format!("n{i}")).collect();
        let nodes: Vec<_> = names.iter().map(|s| node(s, "")).collect();
        let edges: Vec<_> = (1..n)
            .map(|i| edge(&format!("e{i}"), &names[i - 1], &names[i]))
            .collect();
        simple_graph(&index_model(&nodes, &edges).expect("fits"))
    }

    #[test]
    fn a_path_of_five_pairs_up_left_to_right() {
        let c = coarsen(&path(5), 5);
        assert_eq!(c.map, [0, 0, 1, 1, 2]);
        assert_eq!(c.node_count, 3);
        assert_eq!(
            (c.graph.lo.clone(), c.graph.hi.clone()),
            (vec![0, 1], vec![1, 2])
        );
    }

    #[test]
    fn an_edgeless_graph_does_not_coarsen() {
        let c = coarsen(&path(1), 1);
        assert_eq!((c.map, c.node_count), (vec![0], 1));
        assert!(c.graph.lo.is_empty());
    }

    #[test]
    fn a_star_matches_only_the_lowest_leaf() {
        let names = ["h", "a", "b", "c"];
        let nodes: Vec<_> = names.iter().map(|s| node(s, "")).collect();
        let edges = [
            edge("e1", "h", "a"),
            edge("e2", "h", "b"),
            edge("e3", "h", "c"),
        ];
        let g = simple_graph(&index_model(&nodes, &edges).expect("fits"));
        let c = coarsen(&g, 4);
        assert_eq!(c.map, [0, 0, 1, 2]);
        assert_eq!(
            (c.graph.lo.clone(), c.graph.hi.clone()),
            (vec![0, 0], vec![1, 2])
        );
    }
}

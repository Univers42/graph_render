use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};

/// Two triangles joined by one bridge edge — the textbook case: Louvain should keep
/// each triangle together and the bridge should not merge them.
fn two_cliques() -> Topology {
    let ids = ["a0", "a1", "a2", "b0", "b1", "b2"];
    let nodes: Vec<_> = ids.iter().map(|id| node(id, "")).collect();
    let mut edges = vec![
        edge("a01", "a0", "a1"),
        edge("a12", "a1", "a2"),
        edge("a20", "a2", "a0"),
        edge("b01", "b0", "b1"),
        edge("b12", "b1", "b2"),
        edge("b20", "b2", "b0"),
    ];
    edges.push(edge("bridge", "a0", "b0"));
    index_model(&nodes, &edges).expect("fits")
}

#[test]
fn two_cliques_stay_separate_communities_across_the_bridge() {
    let t = two_cliques();
    let labels = louvain(&t);
    assert_eq!(
        labels[0..3]
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1
    );
    assert_eq!(
        labels[3..6]
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        1
    );
    assert_ne!(labels[0], labels[3], "the two cliques must not merge");
}

#[test]
fn modularity_of_the_clean_partition_is_positive_and_higher_than_one_lump() {
    let t = two_cliques();
    let good = louvain(&t);
    let one_lump = vec![0u32; 6];
    assert_eq!(
        modularity(&t, &one_lump),
        0.0,
        "one community must always score exactly zero"
    );
    assert!(modularity(&t, &good) > modularity(&t, &one_lump));
    assert!(modularity(&t, &good) > 0.0);
    // fixtures/analysis/two-cliques.json pins this exact value.
    assert_eq!(modularity(&t, &good), 0.3571428571428571);
}

#[test]
fn an_edgeless_graph_gives_every_node_its_own_community_and_zero_modularity() {
    let nodes = [node("a", ""), node("b", "")];
    let t = index_model(&nodes, &[]).expect("fits");
    assert_eq!(louvain(&t), [0, 1]);
    assert_eq!(modularity(&t, &[0, 1]), 0.0);
}

#[test]
fn repeated_runs_agree_bit_for_bit() {
    let t = two_cliques();
    assert_eq!(louvain(&t), louvain(&t));
}

/// Regression for the BLOCKER finding (phase-07 review): `move_node` took 6 loose
/// parameters, over the house's <=4-parameter limit. This drives it through the
/// bundled `LouvainState` instead, on the smallest case that actually moves a node —
/// two singleton communities across one edge must merge into one.
#[test]
fn move_node_via_bundled_state_merges_two_connected_singletons() {
    let adjacency = vec![vec![(1u32, 1.0)], vec![(0u32, 1.0)]];
    let degree = vec![1.0, 1.0];
    let mut community = vec![0u32, 1u32];
    let mut total = degree.clone();
    let mut state = LouvainState {
        adjacency: &adjacency,
        degree: &degree,
        community: &mut community,
        total: &mut total,
        m: 1.0,
    };
    let moved = move_node(1, &mut state);
    assert!(
        moved,
        "joining the only neighbour's community is a strict gain"
    );
    assert_eq!(community, [0, 0]);
    assert_eq!(total, [2.0, 0.0]);
}

/// U26 (`docs/reviews/review-core-post.md`): `sort_unstable_by_key` cannot reorder
/// ties because there are none: `find` merges every community into one entry first.
#[test]
fn neighbor_weights_holds_one_entry_per_community_so_the_sort_has_no_ties() {
    let row = vec![(3u32, 1.0), (1, 2.0), (2, 4.0), (0, 8.0), (3, 16.0)];
    let adjacency = vec![row, vec![], vec![], vec![]];
    let community = [0u32, 2, 1, 2];
    let weights = neighbor_weights(0, &adjacency, &community);
    assert_eq!(weights, [(1, 4.0), (2, 19.0)]);
}

fn weighted(id: &str, a: &str, b: &str, weight: f64) -> crate::records::EdgeRecord {
    let mut e = edge(id, a, b);
    e.strength = weight;
    e
}

/// R25 (`docs/reviews/review-core-post.md`): networkx's `G.degree` counts a self-loop
/// twice, so `{a}` carries degree 3 here, not 2. The value is networkx 3.6's own
/// `community.modularity(G, [{"a"}, {"b"}], weight="weight")`, run in ge-python-oracle.
#[test]
fn modularity_counts_a_self_loop_twice_like_networkx() {
    let nodes = [node("a", ""), node("b", "")];
    let edges = [weighted("ab", "a", "b", 1.0), weighted("aa", "a", "a", 1.0)];
    let t = index_model(&nodes, &edges).expect("fits");
    assert_eq!(modularity(&t, &[0, 1]), -0.125);
}

/// R25, the gain side: `louvain.py:265` reads `G.degree(weight="weight")` as `du`
/// (`:290-292`, `:308-311`), a self-loop counted twice. Pinned to networkx 3.6's first
/// `louvain_partitions` level with an identity shuffle, so its visit order is the
/// dense index order this port uses; counting the loop once merges all three nodes.
#[test]
fn louvain_counts_a_self_loop_twice_like_networkx() {
    let nodes = [node("n0", ""), node("n1", ""), node("n2", "")];
    let edges = [
        weighted("n01", "n0", "n1", 1.0),
        weighted("n12", "n1", "n2", 1.0),
        weighted("n00", "n0", "n0", 0.5),
    ];
    let t = index_model(&nodes, &edges).expect("fits");
    assert_eq!(louvain(&t), [0, 1, 1]);
}

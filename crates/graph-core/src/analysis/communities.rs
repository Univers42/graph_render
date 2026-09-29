//! Community detection (`prompts/phase-07-analysis.md` step 5). References: `SciGraphs/
//! engine/scigraphs_engine/communities.py` (uses greedy modularity or agglomeration
//! instead of Louvain there, precisely because Louvain is randomised and order-dependent
//! — the exact concern this port removes) and networkx 3.6 `algorithms/community/
//! louvain.py` for the modularity-gain formula this is checked against by hand.
//!
//! **Scope, recorded as a deviation** (`docs/measurements/phase07-analysis.md`): this is
//! Louvain's *local-moving* phase (Blondel et al. 2008, phase 1) iterated to a fixed
//! point, without the aggregation phase that re-runs it over communities-as-nodes. One
//! level already optimises modularity by the same gain rule; the aggregated second
//! level is future work, not claimed under a name it does not yet meet.
//!
//! **Determinism, a further deviation from the phase's own suggestion:** dense-index
//! visit order and a total tie-break (ascending community id; a move needs a *strict*
//! gain, so the first-found best strictly dominates any later tie) replace networkx's
//! seeded shuffle — **no RNG at all**, rather than threading a seed for no measured
//! benefit. If a future revision needs one, `synthetic::Mulberry32` is the house RNG,
//! threaded explicitly, never ambient.
//!
//! Ponytail: Louvain is a heuristic and order-dependent. Failing input: a graph with
//! near-tied modularity gains between two moves. Direction: cosmetic, never dangerous —
//! it returns *a* valid partition, just not necessarily the highest-modularity one.
//! Escape hatch: none needed for reproducibility (this port has no seed to vary), but a
//! different fixed visit order is a different, equally valid answer.

use crate::analysis::components::canonicalize;
use crate::index::Topology;

/// Modularity `Q` of `membership` over the undirected weighted projection (edge weight
/// `strength`, direction ignored — the classical definition). `0.0` for a graph with no
/// edge weight at all.
///
/// Newman's `Q = (1/2m) sum_ij [A_ij - k_i k_j/2m] delta(c_i,c_j)` splits into an edge
/// term (`internal`, summed only where an edge exists) and a null-model term that runs
/// over *every* same-community pair whether or not it is an edge — `sum_c (K_c/2m)^2`,
/// `K_c` the community's total degree — collapsing the O(n^2) pair sum into one pass per
/// community. Summing `k_i k_j/2m` only over existing edges, as an earlier draft did,
/// silently drops every same-community *non*-edge pair and overstates `Q`; the
/// `one_lump` fixture (every node one community, no edge excluded) is what caught it: a
/// single community must always score exactly `0.0`, and it did not until this form was
/// used.
pub fn modularity(topology: &Topology, membership: &[u32]) -> f64 {
    let (adjacency, total_weight) = undirected_adjacency(topology);
    if total_weight <= 0.0 {
        return 0.0;
    }
    let degree = weighted_degree(&adjacency);
    let m2 = 2.0 * total_weight;
    let communities = membership.iter().copied().max().map_or(0, |m| m + 1) as usize;
    let mut community_degree = vec![0.0; communities];
    let mut internal = 0.0;
    for (u, neighbors) in adjacency.iter().enumerate() {
        community_degree[membership[u] as usize] += degree[u];
        for &(v, w) in neighbors {
            if membership[u] == membership[v as usize] {
                internal += w;
            }
        }
    }
    let null_model: f64 = community_degree.iter().map(|k| (k / m2).powi(2)).sum();
    internal / m2 - null_model
}

/// Louvain's local-moving phase (module doc: scope). Every node starts in its own
/// community; repeatedly, in dense-index order, each node joins the neighbour community
/// (or stays) giving the strictest positive modularity gain, until a full pass moves
/// nothing. Community ids are canonicalised the same way `components` numbers its own.
pub fn louvain(topology: &Topology) -> Vec<u32> {
    let (adjacency, total_weight) = undirected_adjacency(topology);
    let n = adjacency.len();
    let mut community: Vec<u32> = (0..n as u32).collect();
    if total_weight <= 0.0 {
        return canonicalize(&community);
    }
    let degree = weighted_degree(&adjacency);
    let mut total = degree.clone();
    {
        let mut state = LouvainState {
            adjacency: &adjacency,
            degree: &degree,
            community: &mut community,
            total: &mut total,
            m: total_weight,
        };
        let mut moved = true;
        while moved {
            moved = false;
            for u in 0..n {
                moved |= move_node(u, &mut state);
            }
        }
    }
    canonicalize(&community)
}

/// The mutable Louvain bookkeeping [`move_node`] reads and updates for one node, bundled
/// into a struct rather than passed as five loose parameters — `move_node` plus this
/// state stays inside the house's <=4-parameter limit (`refactor-rust.md`). `adjacency`
/// and `degree` are read-only per pass; `community` and `total` (each community's
/// running weighted degree) are what a move mutates.
struct LouvainState<'a> {
    adjacency: &'a [Vec<(u32, f64)>],
    degree: &'a [f64],
    community: &'a mut [u32],
    total: &'a mut [f64],
    m: f64,
}

/// One node's move: removes it from its community's totals, finds the best neighbour
/// community by modularity gain (a *strict* improvement over staying; ties among
/// neighbours favour the lower community id, since `weights` is scanned ascending and
/// only a strictly larger gain replaces the current best), and re-inserts it there.
fn move_node(u: usize, state: &mut LouvainState) -> bool {
    let (home, du) = (state.community[u], state.degree[u]);
    state.total[home as usize] -= du;
    let weights = neighbor_weights(u, state.adjacency, state.community);
    let home_weight = weights
        .iter()
        .find(|&&(c, _)| c == home)
        .map_or(0.0, |&(_, w)| w);
    let m = state.m;
    let remove_cost = -home_weight / m + state.total[home as usize] * du / (2.0 * m * m);
    let mut best = (home, 0.0f64);
    for &(c, w) in &weights {
        let gain = remove_cost + w / m - state.total[c as usize] * du / (2.0 * m * m);
        if gain > best.1 {
            best = (c, gain);
        }
    }
    state.total[best.0 as usize] += du;
    state.community[u] = best.0;
    best.0 != home
}

/// `u`'s edge weight into every neighbour community it currently has an edge to (a
/// self-loop excluded, as networkx's own `nbrs` does), ascending by community id — the
/// fixed order [`move_node`]'s tie-break relies on.
fn neighbor_weights(u: usize, adjacency: &[Vec<(u32, f64)>], community: &[u32]) -> Vec<(u32, f64)> {
    let mut weights: Vec<(u32, f64)> = Vec::new();
    for &(v, w) in &adjacency[u] {
        if v as usize == u {
            continue;
        }
        let c = community[v as usize];
        match weights.iter_mut().find(|(existing, _)| *existing == c) {
            Some(entry) => entry.1 += w,
            None => weights.push((c, w)),
        }
    }
    weights.sort_unstable_by_key(|&(c, _)| c);
    weights
}

fn weighted_degree(adjacency: &[Vec<(u32, f64)>]) -> Vec<f64> {
    adjacency
        .iter()
        .map(|n| n.iter().map(|&(_, w)| w).sum())
        .collect()
}

/// The undirected weighted projection: `strength` symmetrised, direction ignored, plus
/// the graph's total edge weight (each edge counted once).
fn undirected_adjacency(topology: &Topology) -> (Vec<Vec<(u32, f64)>>, f64) {
    let n = topology.node_count() as usize;
    let mut adjacency = vec![Vec::new(); n];
    let edges = topology.edges();
    let mut total = 0.0;
    for i in 0..topology.edge_count() as usize {
        let (a, b, w) = (edges.source[i], edges.target[i], edges.strength[i]);
        adjacency[a as usize].push((b, w));
        if a != b {
            adjacency[b as usize].push((a, w));
        }
        total += w;
    }
    (adjacency, total)
}

#[cfg(test)]
mod tests {
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
}

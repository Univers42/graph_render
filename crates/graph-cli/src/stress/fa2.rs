//! `layout.forceatlas2.barnes_hut` against the exact `layout.forceatlas2` it approximates,
//! on the stress gate's own graphs, metric and verdict: no case more than [`MARGIN`] below
//! the exact sum's correlation, and the median not below it either. The exact layout is
//! the oracle here, so this runs in the merge floor with no harness beside it.

use super::MARGIN;
use super::cases::case;
use super::metric::{Graph, correlate};
use graph_core::layout::forceatlas2::{Fa2Params, ForceAtlas2, ForceAtlas2BarnesHut};
use graph_core::{REFERENCE_DEGREE, Stage, gate_node_count, index_model, seeded_model};

/// Every 19th gate seed: 32 graphs of 2 to 591 nodes.
const SEEDS: [u32; 32] = {
    let mut seeds = [0; 32];
    let mut k = 0;
    while k < 32 {
        seeds[k] = k as u32 * 19;
        k += 1;
    }
    seeds
};

/// The approximate layout's correlation minus the exact one's, on one gate seed.
fn margin(seed: u32) -> Option<f64> {
    let (nodes, edges) = seeded_model(seed, gate_node_count(seed), REFERENCE_DEGREE);
    let topology = index_model(&nodes, &edges).expect("a gate model indexes");
    let params = Fa2Params::default();
    let exact = ForceAtlas2::run(&topology, &params).expect("exact runs");
    let tree = ForceAtlas2BarnesHut::run(&topology, &params).expect("tree runs");
    let (exact, tree) = (case(seed, &topology, &exact), case(seed, &topology, &tree));
    let graph = Graph::from_edges(&exact.edges);
    Some(correlate(&graph, &tree.positions)? - correlate(&graph, &exact.positions)?)
}

#[test]
fn the_tree_layout_is_different_but_not_worse_than_the_exact_sum() {
    let mut margins: Vec<f64> = SEEDS.iter().filter_map(|&seed| margin(seed)).collect();
    margins.sort_by(f64::total_cmp);
    let median = margins[margins.len() / 2];
    eprintln!(
        "fa2 stress: {} cases, worst {:+.5}, median {median:+.5}",
        margins.len(),
        margins[0]
    );
    assert!(
        margins.len() >= 28,
        "only {} cases correlated",
        margins.len()
    );
    assert!(
        margins[0] >= MARGIN,
        "worst case {:+.5} below {MARGIN}",
        margins[0]
    );
    assert!(median >= MARGIN, "median {median:+.5} below {MARGIN}");
}

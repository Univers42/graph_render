//! Golden boxes for the ordinary shapes (a deep chain, a wide fan, an unbalanced tree and
//! two fixtures), produced by running d3-hierarchy@3.1.2 (`treemapSquarify`, size 1x1,
//! the call sequence `golden.rs` states) and compared with `f64::to_bits`.

use super::*;

mod data_a;
use data_a::*;

mod data_b;
use data_b::*;

fn assert_boxes(topology: &Topology, want: &[[u64; 4]], label: &str) {
    let hierarchy = Hierarchy::of(topology).expect("fits");
    let boxes = compute(topology, &hierarchy);
    for (v, edges) in want.iter().enumerate() {
        let r = boxes.rect(v as u32);
        assert_eq!(
            [r.x0, r.y0, r.x1, r.y1].map(f64::to_bits),
            *edges,
            "{label}: node {v} differs from d3-hierarchy 3.1.2"
        );
    }
}

fn synthetic(weights: &[f64], pairs: &[(usize, usize)]) -> Topology {
    let nodes: Vec<_> = weights
        .iter()
        .enumerate()
        .map(|(i, w)| weighted(&format!("v{i}"), *w))
        .collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, (p, c))| {
            tree(
                &format!("e{i}"),
                &format!("v{p}"),
                &format!("v{c}"),
                "parent_of",
            )
        })
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

#[test]
fn a_deep_chain_matches_d3_bit_for_bit() {
    let pairs: Vec<_> = (0..7).map(|i| (i, i + 1)).collect();
    let t = synthetic(&[1.0, 2.0, 0.5, 3.0, 1.0, 4.0, 1.0, 2.0], &pairs);
    assert_boxes(&t, &CHAIN8, "chain8");
}

#[test]
fn a_ten_leaf_fan_matches_d3_bit_for_bit() {
    let pairs: Vec<_> = (1..=10).map(|i| (0, i)).collect();
    let w = [1.0, 3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0, 5.0, 3.0];
    assert_boxes(&synthetic(&w, &pairs), &FAN10, "fan10");
}

#[test]
fn an_unbalanced_tree_matches_d3_bit_for_bit() {
    let pairs = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 4),
        (4, 5),
        (0, 6),
        (6, 7),
        (6, 8),
        (6, 9),
    ];
    let w = [1.0, 2.0, 1.0, 5.0, 1.0, 3.0, 1.0, 2.0, 4.0, 1.0];
    assert_boxes(&synthetic(&w, &pairs), &UNBALANCED, "unbalanced");
}

#[test]
fn the_balanced_fixture_matches_d3_bit_for_bit() {
    assert_boxes(
        &from_fixture("tree-balanced"),
        &TREE_BALANCED,
        "tree-balanced",
    );
}

#[test]
fn the_degenerate_fixture_matches_d3_bit_for_bit() {
    assert_boxes(
        &from_fixture("tree-degenerate"),
        &TREE_DEGENERATE,
        "tree-degenerate",
    );
}

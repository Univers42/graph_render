use super::*;

#[test]
fn a_seeds_line_carries_a_tree_and_both_layouts_arrays_the_same_length() {
    let line = fixture_line(7).expect("valid");
    let tree = &line["tree"];
    assert!(tree["children"].is_array());
    let n = graph_core::gate_node_count(7) as usize;
    for face in ["tidy", "treemap"] {
        let x = line[face]["x"].as_array().expect("x");
        assert_eq!(x.len(), n, "{face}");
    }
    assert_eq!(line["treemap"]["w"].as_array().expect("w").len(), n);
}

/// The gate model's remix scrambles hierarchy orientation freely, so every seed in this
/// small range draws two or more real roots (checked directly, not assumed).
#[test]
fn the_virtual_root_carries_no_id_and_no_weight_when_there_are_two_roots() {
    for seed in 0..10 {
        let line = fixture_line(seed).expect("valid");
        assert!(line["tree"]["id"].is_null(), "seed {seed}: {line}");
        assert!(line["tree"]["weight"].is_null(), "seed {seed}: {line}");
    }
}

#[test]
fn every_real_node_in_the_tree_has_a_finite_weight_and_a_dense_index_id() {
    let line = fixture_line(3).expect("valid");
    let mut seen = std::collections::BTreeSet::new();
    fn walk(node: &serde_json::Value, seen: &mut std::collections::BTreeSet<u64>) {
        if let Some(id) = node["id"].as_u64() {
            seen.insert(id);
            assert!(node["weight"].as_f64().expect("finite").is_finite());
        }
        for child in node["children"].as_array().expect("children") {
            walk(child, seen);
        }
    }
    walk(&line["tree"], &mut seen);
    let n = graph_core::gate_node_count(3) as u64;
    assert_eq!(seen, (0..n).collect());
}

/// Two seeds' fixtures must not be byte-identical: the seed reaches the tree and both
/// layouts, not just the node count.
#[test]
fn two_seeds_six_hundred_apart_share_a_node_count_but_draw_different_fixtures() {
    let a = fixture_line(11).expect("valid");
    let b = fixture_line(611).expect("valid");
    assert_eq!(
        graph_core::gate_node_count(11),
        graph_core::gate_node_count(611)
    );
    assert_ne!(a, b);
}

//! The tests for [`shapes`].

use super::{CATERPILLAR9, CHAIN8, FAN10, UNBALANCED, assert_golden, build, nodes, tree};

#[test]
fn a_deep_chain_matches_d3_hierarchy_bit_for_bit() {
    let mut ids: Vec<String> = (0..8).map(|i| format!("n{i}")).collect();
    let mut edges = Vec::new();
    for i in 1..8 {
        edges.push(tree(
            &format!("e{i}"),
            &ids[i - 1].clone(),
            &ids[i].clone(),
            "parent_of",
        ));
    }
    let refs: Vec<&str> = ids.iter_mut().map(|s| s.as_str()).collect();
    assert_golden(&build(&nodes(&refs), &edges), &refs, &CHAIN8, "chain8");
}

#[test]
fn a_ten_leaf_fan_matches_d3_hierarchy_bit_for_bit() {
    let mut ids: Vec<String> = vec!["r".into()];
    let mut edges = Vec::new();
    for i in 0..10 {
        let leaf = format!("l{i}");
        edges.push(tree(&format!("e{i}"), "r", &leaf, "parent_of"));
        ids.push(leaf);
    }
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_golden(&build(&nodes(&refs), &edges), &refs, &FAN10, "fan10");
}

#[test]
fn an_unbalanced_tree_matches_d3_hierarchy_bit_for_bit() {
    let mut ids: Vec<String> = vec!["r".into(), "a".into(), "b".into()];
    let mut edges = vec![
        tree("r-a", "r", "a", "parent_of"),
        tree("r-b", "r", "b", "parent_of"),
    ];
    for i in 0..6 {
        let child = format!("a{i}");
        let parent = if i == 0 {
            "a".to_string()
        } else {
            format!("a{}", i - 1)
        };
        edges.push(tree(&format!("ea{i}"), &parent, &child, "parent_of"));
        ids.push(child);
    }
    for i in 0..5 {
        let child = format!("b{i}");
        edges.push(tree(&format!("eb{i}"), "b", &child, "parent_of"));
        ids.push(child);
    }
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_golden(
        &build(&nodes(&refs), &edges),
        &refs,
        &UNBALANCED,
        "unbalanced",
    );
}

/// The spine runs `c_i → s_i → c_{i+1}`: each `c_i` carries one leaf `s_i` and hands the
/// spine on to it, so the contour walk's two sides descend together and one always runs
/// out first. Note the second edge of each step parents `c_{i+1}` to `s_i`, not to
/// `c_i` — that is what makes it a caterpillar rather than a fan of leaves.
#[test]
fn a_caterpillar_matches_d3_hierarchy_bit_for_bit() {
    let mut ids: Vec<String> = vec!["c0".into()];
    let mut edges = Vec::new();
    for i in 1..9 {
        let c = format!("c{i}");
        let s = format!("s{i}");
        let from = if i == 1 {
            "c0".to_string()
        } else {
            format!("s{}", i - 1)
        };
        edges.push(tree(&format!("ec{i}"), &from, &c, "parent_of"));
        edges.push(tree(&format!("es{i}"), &c, &s, "parent_of"));
        ids.push(c);
        ids.push(s);
    }
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    assert_golden(
        &build(&nodes(&refs), &edges),
        &refs,
        &CATERPILLAR9,
        "caterpillar9",
    );
}

//! The review's simplify findings that break the journal's own invariants
//! (`docs/reviews/review-core-post.md` R7, U25), plus two found while fixing them: a
//! two-node component and a community step's links. `tests.rs` holds the originals.

use super::scaling_tests::graph;
use super::tests::path6;
use super::*;

/// Every node's representative is drawn and is its own representative, so a front that
/// follows `representative` always lands on something it draws.
fn assert_representatives_survive(s: &Simplified) {
    for (node, &rep) in s.representative.iter().enumerate() {
        assert_eq!(
            s.visible[rep as usize], 1,
            "node {node} -> hidden {rep}: {s:?}"
        );
        assert_eq!(
            s.representative[rep as usize], rep,
            "node {node} -> {rep}: {s:?}"
        );
    }
}

/// Every link of every step of `kind` names two drawn nodes that are their own
/// representatives.
fn assert_links_survive(s: &Simplified, kind: Kind) {
    for step in s.steps.iter().filter(|step| step.kind == kind) {
        for &(a, b) in &step.links {
            for end in [a, b] {
                assert_eq!(s.visible[end as usize], 1, "link {a}-{b} of {step:?}");
                assert_eq!(s.representative[end as usize], end, "link {a}-{b}");
            }
        }
    }
}

/// R7. Leaves fold first, so a chain whose branch node was a leaf ends on a hidden
/// node: the chain must not be contracted onto it.
#[test]
fn a_chain_never_contracts_onto_a_folded_leaf() {
    let t = path6();
    let plan = Plan {
        fold_leaves: true,
        contract_chains: true,
        collapse_communities: false,
    };
    let s = simplify(&t, &plan);
    assert_representatives_survive(&s);
    assert_links_survive(&s, Kind::Chain);
    assert!(s.visible.contains(&1), "something is still drawn: {s:?}");
}

/// Found while fixing R7: two leaves joined to each other. `fold_leaves` says the pair
/// folds `1` into `0`; before, `0` folded into `1` and then `1` into the hidden `0`, and
/// nothing was drawn.
#[test]
fn a_two_node_component_keeps_its_lower_index() {
    use crate::index::index_model;
    use crate::records::build::{edge, node};
    let t = index_model(&[node("a", "db"), node("b", "db")], &[edge("e", "a", "b")]).expect("fits");
    let plan = Plan {
        fold_leaves: true,
        ..Plan::nothing()
    };
    let s = simplify(&t, &plan);
    assert_representatives_survive(&s);
    assert_eq!(s.visible, vec![1, 0]);
    assert_eq!(s.steps.len(), 1, "{:?}", s.steps);
    assert_eq!(
        (s.steps[0].representative, &s.steps[0].nodes),
        (0, &vec![1])
    );
}

/// U25. The community pass had R7's hole: a community whose lowest index the leaf pass
/// folded was represented by that hidden node. `n0-n1` plus the triangle `1-2-3`.
#[test]
fn a_community_never_collapses_onto_a_folded_leaf() {
    let t = graph(4, [(0, 1), (1, 2), (2, 3), (3, 1)].into_iter());
    let s = simplify(&t, &Plan::all());
    assert_representatives_survive(&s);
    assert_links_survive(&s, Kind::Community);
}

/// Found while fixing R24: a community step's links are the external edges that touch
/// it, not every external edge of the graph. Three triangles in a row, `0-1-2`, `3-4-5`
/// and `6-7-8`, joined by `2-3` and `5-6`.
#[test]
fn a_community_step_links_only_its_own_external_edges() {
    let triangles = [
        (0, 1),
        (1, 2),
        (2, 0),
        (3, 4),
        (4, 5),
        (5, 3),
        (6, 7),
        (7, 8),
        (8, 6),
    ];
    let t = graph(9, triangles.into_iter().chain([(2, 3), (5, 6)]));
    let plan = Plan {
        collapse_communities: true,
        ..Plan::nothing()
    };
    let s = simplify(&t, &plan);
    let links: Vec<_> = s
        .steps
        .iter()
        .map(|step| (step.representative, step.links.clone()))
        .collect();
    assert_eq!(
        links,
        vec![
            (0, vec![(0, 3)]),
            (3, vec![(0, 3), (3, 6)]),
            (6, vec![(3, 6)])
        ]
    );
    assert_links_survive(&s, Kind::Community);
}

use super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};

/// A path of 6: nodes 0..6 with 0 and 5 the only branch nodes, 1..4 degree 2.
fn path6() -> Topology {
    let nodes: Vec<_> = (0..6).map(|i| node(&format!("n{i}"), "db")).collect();
    let edges: Vec<_> = (1..6)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

/// The gate row, in the shape the phase's gate runs it: whatever the plan removed,
/// [`restore`] gives the original graph back, node for node and edge for edge, from
/// the journal alone.
#[test]
fn simplify_reversible_restores_the_original_nodes_edges_and_representatives() {
    let t = path6();
    let original = (
        vec![1_u8; t.node_count() as usize],
        vec![1_u8; t.edge_count() as usize],
        (0..t.node_count()).collect::<Vec<u32>>(),
    );
    for plan in [
        Plan::nothing(),
        Plan::all(),
        Plan {
            fold_leaves: true,
            ..Plan::nothing()
        },
        Plan {
            contract_chains: true,
            ..Plan::nothing()
        },
        Plan {
            collapse_communities: true,
            ..Plan::nothing()
        },
    ] {
        let s = simplify(&t, &plan);
        let back = restore(&s);
        assert_eq!(
            (back.visible, back.edges, back.representative),
            original,
            "{plan:?}"
        );
    }
}

/// Reversibility on a graph with every kind of node in it: leaves, a chain, a ring
/// and a dense core, so each pass has something to remove and nothing overlaps.
#[test]
fn simplify_reversible_holds_on_a_mixed_graph() {
    let mut nodes: Vec<_> = (0..15).map(|i| node(&format!("n{i}"), "db")).collect();
    nodes.push(node("leaf", "db"));
    let ids: Vec<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let pairs = [
        (0, 1),
        (1, 2),
        (2, 3),
        (3, 0), // a ring of four: no branch node to contract to
        (0, 4),
        (4, 5),
        (5, 0), // a triangle
        (0, 6),
        (6, 7),
        (7, 0), // another triangle
        (1, 8),
        (8, 9),
        (9, 1), // another
        (2, 10),
        (10, 11),
        (11, 2), // another
        (3, 12),
        (12, 13),
        (13, 3), // another
        (4, 15), // node 15 is a leaf
    ];
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, (a, b))| edge(&format!("e{i}"), &ids[*a as usize], &ids[*b as usize]))
        .collect();
    let t = index_model(&nodes, &edges).expect("fits");
    let s = simplify(&t, &Plan::all());
    let back = restore(&s);
    assert_eq!(back.visible, vec![1; t.node_count() as usize]);
    assert_eq!(back.edges, vec![1; t.edge_count() as usize]);
    assert_eq!(
        back.representative,
        (0..t.node_count()).collect::<Vec<u32>>()
    );
    assert_eq!(s.visible[15], 0, "the leaf was folded");
    assert!(
        counts(&s).0 > 0,
        "something was reduced, or the graph proves nothing"
    );
    let kinds: Vec<Kind> = s.steps.iter().map(|step| step.kind).collect();
    assert!(kinds.contains(&Kind::Leaf), "{kinds:?}");
    assert!(
        kinds.windows(2).all(|w| w[0] <= w[1]),
        "the journal is in pass order: {kinds:?}"
    );
    assert!(
        s.steps
            .iter()
            .all(|step| s.visible[step.representative as usize] == 1),
        "every step names a node that survived"
    );
    assert!(
        s.steps
            .iter()
            .all(|step| step.nodes.iter().all(|&node| s.visible[node as usize] == 0))
    );
}

/// A contracted chain looks like an edge: the interior nodes go, the two branch
/// nodes stay, and the journal is what says they are now adjacent.
#[test]
fn a_contracted_chain_is_one_journalled_link_between_its_branch_nodes() {
    let t = path6();
    let s = simplify(
        &t,
        &Plan {
            contract_chains: true,
            ..Plan::nothing()
        },
    );
    let step = s.steps.first().expect("the chain was contracted");
    assert_eq!(step.kind, Kind::Chain);
    assert_eq!(step.nodes, vec![1, 2, 3, 4]);
    assert_eq!(step.links, vec![(0, 5)]);
    assert_eq!(step.edges, vec![0, 1, 2, 3, 4]);
    assert_eq!(s.visible, vec![1, 0, 0, 0, 0, 1]);
    assert_eq!(s.edges, vec![0; 5]);
    assert_eq!(s.representative, vec![0, 0, 0, 0, 0, 5]);
}

/// A bare ring has no branch node, so it is left whole: half a contraction would draw
/// fewer nodes than the graph has and imply an edge that does not exist.
#[test]
fn a_ring_of_degree_two_nodes_is_left_alone() {
    let nodes: Vec<_> = (0..4).map(|i| node(&format!("n{i}"), "db")).collect();
    let edges: Vec<_> = (0..4)
        .map(|i| {
            edge(
                &format!("e{i}"),
                &format!("n{i}"),
                &format!("n{}", (i + 1) % 4),
            )
        })
        .collect();
    let t = index_model(&nodes, &edges).expect("fits");
    let s = simplify(
        &t,
        &Plan {
            contract_chains: true,
            ..Plan::nothing()
        },
    );
    assert!(s.steps.is_empty(), "{:?}", s.steps);
    assert_eq!(s.visible, vec![1; 4]);
}

/// Communities collapse to their lowest dense index, and the links the journal
/// carries are the external edges re-anchored there — the drill-back a front needs.
#[test]
fn a_community_collapses_onto_its_lowest_index_with_its_external_links() {
    // Two triangles joined by one edge: two communities, joined at 2 and 5.
    let nodes: Vec<_> = (0..6).map(|i| node(&format!("n{i}"), "db")).collect();
    let pairs = [(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3), (2, 5)];
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    let t = index_model(&nodes, &edges).expect("fits");
    let s = simplify(
        &t,
        &Plan {
            collapse_communities: true,
            ..Plan::nothing()
        },
    );
    assert_eq!(s.steps.len(), 2, "two communities, both of size three");
    assert!(s.steps.iter().all(|step| step.kind == Kind::Community));
    let first = &s.steps[0];
    assert_eq!(first.representative, 0);
    assert_eq!(first.nodes, vec![1, 2]);
    assert_eq!(
        first.links,
        vec![(0, 3)],
        "the joining edge, re-anchored on 0"
    );
    assert_eq!(first.edges, vec![0, 1, 2], "the triangle's own three edges");
    assert_eq!(
        s.visible,
        vec![1, 0, 0, 1, 0, 0],
        "one node per community survives"
    );
    assert_eq!(s.edges[6], 1, "the joining edge itself is still drawn");
    assert_eq!(s.edges[0], 0, "an edge inside a collapsed community is not");
}

/// Nothing is removed unless a pass is asked for, and the identity plan is the
/// identity: the same graph, the same journal length, node for node.
#[test]
fn the_identity_plan_removes_nothing_and_the_passes_are_ordered() {
    let t = path6();
    let none = simplify(&t, &Plan::nothing());
    assert!((none.steps.is_empty(), counts(&none)) == (true, (0, 0)));
    assert_eq!(none.visible, vec![1; 6]);
    let all = simplify(&t, &Plan::all());
    let kinds: Vec<Kind> = all.steps.iter().map(|s| s.kind).collect();
    assert!(kinds.windows(2).all(|w| w[0] <= w[1]), "{kinds:?}");
}

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

/// Every link of every step of `kind` names two **distinct** drawn nodes that are their
/// own representatives. A link with equal ends is the same node twice, which no front can
/// draw: the reference's coarse level never emits one either, since an intra-community
/// edge — a self-loop is the smallest — is not a super-edge (`simplify.py:216-217`).
fn assert_links_survive(s: &Simplified, kind: Kind) {
    for step in s.steps.iter().filter(|step| step.kind == kind) {
        for &(a, b) in &step.links {
            assert_ne!(a, b, "link {a}-{b} of {step:?} names one node twice");
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

/// Every edge that is still drawn joins two drawn nodes: nothing survives the passes as
/// an edge with nothing behind it.
///
/// **The reference does not decide this case, and it is worth saying so plainly.** Its one
/// self-loop rule is `build_coarse_level`'s (`simplify.py:216-217`: a self-loop's
/// `ca == cb`, so it is never one of the `inter` super-edges), and that covers the coarse
/// level — the community collapse — alone. There is no reference function for leaf folding
/// or for chain contraction at all, so for the two passes below the argument is the
/// journal's own: a drawn edge with no drawn end is nothing a front can draw. The Rust side
/// reached the opposite answer by omission, `edges_between` being only ever asked about
/// `a != b` and `simple::build` dropping self-loops from the adjacency, so a self-loop on a
/// node a pass hides was never journalled and never cleared.
#[test]
fn a_self_loop_on_a_node_a_pass_removes_goes_with_it() {
    let star = graph(4, [(0, 1), (0, 2), (0, 3), (1, 1)].into_iter());
    let path = graph(
        6,
        [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (2, 2)].into_iter(),
    );
    // `Plan::all()` and the single-pass plans both, because under `Plan::all()` the
    // community pass happens to catch a self-loop whose node a chain removed — the
    // self-loop reads as internal to one representative. That is the chain pass's
    // accident to rely on, so the chain is pinned on its own.
    let leaves = Plan {
        fold_leaves: true,
        ..Plan::nothing()
    };
    let chains = Plan {
        contract_chains: true,
        ..Plan::nothing()
    };
    let cases = [
        (&star, Plan::all()),
        (&star, leaves),
        (&path, Plan::all()),
        (&path, chains),
    ];
    let dangling: Vec<String> = cases
        .iter()
        .flat_map(|(t, plan)| {
            let s = simplify(t, plan);
            (0..t.edge_count() as usize)
                .filter(|&e| s.edges[e] == 1)
                .filter(|&e| {
                    [t.edges().source[e], t.edges().target[e]]
                        .iter()
                        .any(|&end| s.visible[end as usize] == 0)
                })
                .map(|e| format!("{e}->{:?}", [t.edges().source[e], t.edges().target[e]]))
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        dangling.is_empty(),
        "drawn edges with a hidden end: {dangling:?}"
    );
}

/// Found while fixing F1's neighbour: a chain step's `links` are written from the branch
/// nodes as they were drawn at chain time (`chain.rs:45`). The community pass may then
/// hide one of them, and before this was fixed nothing rewrote the journal — so the
/// drill-back a front reads named a node it does not draw. `assert_links_survive` existed
/// for this and was only ever called with the community pass off.
///
/// The precondition is asserted, not assumed: if Louvain's partition stops hiding an end
/// of some chain, this test fails loudly instead of quietly passing on nothing. It is
/// stated on the *pre-community* journal and the full-plan masks, which is a fact about
/// the graph and the partition — not about the journal this fix rewrites.
#[test]
fn a_chain_link_is_re_anchored_when_the_community_pass_hides_an_end() {
    // Two triangles `0-1-2` and `5-6-7`, joined by the degree-2 run `2-3-4-5`, so the
    // chain `3-4` contracts to the link `(2, 5)` and Louvain — which finds the three
    // communities `[0,0,0,1,1,2,2,2]` — is free to hide `2` into the community led by 0.
    let pairs = [
        (0, 1),
        (1, 2),
        (2, 0),
        (2, 3),
        (3, 4),
        (4, 5),
        (5, 6),
        (6, 7),
        (7, 5),
    ];
    let t = graph(8, pairs.into_iter());
    let structural = Plan {
        collapse_communities: false,
        ..Plan::all()
    };
    let before = simplify(&t, &structural);
    let ends: Vec<u32> = before
        .steps
        .iter()
        .filter(|step| step.kind == Kind::Chain)
        .flat_map(|step| step.links.iter().copied())
        .flat_map(|(a, b)| [a, b])
        .collect();
    let s = simplify(&t, &Plan::all());
    let hidden: Vec<u32> = ends
        .iter()
        .copied()
        .filter(|&end| s.visible[end as usize] == 0)
        .collect();
    assert!(
        !ends.is_empty() && !hidden.is_empty(),
        "precondition: no chain end is hidden by the community pass, so this proves \
         nothing: ends {ends:?}, hidden {hidden:?}, {s:?}"
    );
    assert_representatives_survive(&s);
    assert_links_survive(&s, Kind::Chain);
    assert_links_survive(&s, Kind::Community);
}

/// A link whose two ends land in the **same** representative. `reanchor_links` maps both
/// ends through `representative`, so a chain step's link `(0, 1)` over one community
/// became `(0, 0)` — one node named twice, which `classify_edges` would never have
/// written (it only pushes a link when `ra != rb`) and which a front cannot draw.
///
/// K4 on `{0,1,2,3}` plus the run `0-4-5-1`: `4` and `5` are the chain's interior, its
/// ends `0` and `1` are both in the clique's community, and the collapse re-anchors the
/// step's link `(0, 1)` onto the representative `0`. Every pass combination is tried, since
/// which step holds the link depends on which ones ran.
#[test]
fn a_link_whose_ends_land_in_one_representative_is_dropped() {
    let clique = [(0, 1), (0, 2), (0, 3), (1, 2), (1, 3), (2, 3)];
    let t = graph(6, clique.into_iter().chain([(0, 4), (4, 5), (5, 1)]));
    let mut seen = 0;
    for leaves in [false, true] {
        for chains in [false, true] {
            for communities in [false, true] {
                let plan = Plan {
                    fold_leaves: leaves,
                    contract_chains: chains,
                    collapse_communities: communities,
                };
                let s = simplify(&t, &plan);
                for kind in [Kind::Leaf, Kind::Chain, Kind::Community] {
                    assert_links_survive(&s, kind);
                }
                seen += s.steps.iter().map(|step| step.links.len()).sum::<usize>();
            }
        }
    }
    assert!(
        seen > 0,
        "no link at all, so the case proves nothing: {:?}",
        simplify(&t, &Plan::all())
    );
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

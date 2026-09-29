//! Phase 2 on its own: [`Lr::test_all`], the left-right constraint pass, read one
//! table at a time.
//!
//! The integration tests can only say "planar" or "not planar" about this phase, and
//! almost every graph in the suite is accepted by it — so a mutation that resolves a
//! side wrongly, or points a `reference` at the wrong return edge, is invisible to
//! them. These tests pin the tables phase 2 *leaves behind*, which is where its whole
//! effect is: the `reference` chain that phase 3 walks, the `side` flags, and an
//! emptied conflict stack.
//!
//! The family is the two-apex suspension ([`apexed_path`]), because a path, a star, a
//! cycle, a fan, a grid, a wheel and an Apollonian network all leave phase 2 with
//! nothing to do: no conflict pair is ever pushed, so `add_constraints`,
//! `merge_lower_conflicts` and the `while` loop in `remove_back_edges` never run.

use super::super::super::adjacency::Adjacency;
use super::super::too_dense;
use super::*;

/// Two independent apexes over a path `0..span`, each joined to every path node and not
/// to each other. Planar by construction: the path on a line, one apex above, one below.
fn apexed_path(span: u32) -> (u32, Vec<(u32, u32)>) {
    let mut edges: Vec<(u32, u32)> = (0..span.saturating_sub(1)).map(|i| (i, i + 1)).collect();
    for apex in [span, span + 1] {
        edges.extend((0..span).map(|v| (v, apex)));
    }
    (span + 2, edges)
}

/// `K_{a,b}`: nodes `0..a` against `a..a + b`.
fn complete_bipartite(a: u32, b: u32) -> Vec<(u32, u32)> {
    (0..a)
        .flat_map(|i| (0..b).map(move |j| (i, a + j)))
        .collect()
}

/// Every `(slot, target)` in `reference`, in ascending slot order — the chain phase 3
/// will walk, written compactly because a `Vec<Option<u32>>` of 34 entries is unreadable.
fn links(reference: &[Option<u32>]) -> Vec<(usize, u32)> {
    reference
        .iter()
        .enumerate()
        .filter_map(|(i, &t)| t.map(|t| (i, t)))
        .collect()
}

/// Every slot marked side `-1`, ascending.
fn left_of_the_middle(side: &[i8]) -> Vec<usize> {
    side.iter()
        .enumerate()
        .filter_map(|(i, &s)| (s == -1).then_some(i))
        .collect()
}

/// One phase-2 read-back case: the path span, the `reference` links phase 3 will walk,
/// and the slots phase 2 marked side `-1`.
type Phase2Case = (u32, Vec<(usize, u32)>, Vec<usize>);

/// Phase 2 on a two-apex path. The `reference` chain and the single `-1` are the whole
/// of what this phase produced: one `remove_back_edges` call found a conflict pair whose
/// `lowest` sat exactly at its parent node's height, dropped it off the stack, and
/// flagged the pair's left interval's lowest return edge `-1`. Change the lowpoint
/// comparison, the trim, the side constant or the reference target and these lists
/// change; a mutation that skipped the phase entirely leaves them empty.
#[test]
fn a_two_apex_path_resolves_exactly_one_side_and_the_return_edge_chain() {
    let cases: [Phase2Case; 4] = [
        (3, vec![(4, 10), (8, 11), (9, 14), (13, 10)], vec![11]),
        (
            4,
            vec![
                (4, 14),
                (8, 19),
                (12, 16),
                (13, 20),
                (16, 15),
                (18, 14),
                (20, 19),
            ],
            vec![15],
        ),
        (
            5,
            vec![
                (4, 18),
                (8, 24),
                (12, 25),
                (16, 21),
                (17, 26),
                (20, 19),
                (21, 20),
                (23, 18),
                (25, 24),
                (26, 25),
            ],
            vec![19],
        ),
        (
            6,
            vec![
                (4, 22),
                (8, 29),
                (12, 30),
                (16, 31),
                (20, 26),
                (21, 32),
                (24, 23),
                (25, 24),
                (26, 25),
                (28, 22),
                (30, 29),
                (31, 30),
                (32, 31),
            ],
            vec![23],
        ),
    ];
    for (span, expected_links, expected_left) in cases {
        let (n, edges) = apexed_path(span);
        let adjacency = Adjacency::simple(n, &edges);
        let mut lr = Lr::new(&adjacency);
        lr.orient_all();
        lr.order_by_nesting_depth();
        assert!(lr.test_all(), "span {span}: a two-apex path is planar");
        assert_eq!(links(&lr.reference), expected_links, "span {span}");
        assert_eq!(left_of_the_middle(&lr.side), expected_left, "span {span}");
        assert!(
            lr.stack.is_empty(),
            "span {span}: every conflict pair must be unwound again"
        );
    }
}

/// The negative control on the same phase, and the only one in the suite: `K3,3` sits
/// *under* the `3n - 6` bound (9 edges against 12), so it is the LR test itself that
/// has to reject it, and `test_all` is where the conflict is detected. `K5` cannot play
/// this part — at 10 edges it is over its own bound of 9, so the edge count refuses it
/// before this phase ever runs.
#[test]
fn k33_is_refused_by_the_conflict_and_not_by_the_edge_bound() {
    let edges = complete_bipartite(3, 3);
    assert!(
        !too_dense(6, 9),
        "9 < 3 * 6 - 6 = 12, so the bound lets it through"
    );
    let adjacency = Adjacency::simple(6, &edges);
    let mut lr = Lr::new(&adjacency);
    lr.orient_all();
    lr.order_by_nesting_depth();
    assert!(
        !lr.test_all(),
        "K3,3 is not planar, and the DFS itself must be the one to say so"
    );
}

/// A pure tree — every edge a tree edge, so there is no return edge anywhere — gives
/// phase 2 nothing to constrain, and it must leave the tables it did not need completely
/// alone. This is the negative control on the opposite side: a mutation that wired a
/// `reference` or flagged a `side` on the way through would show up here as a non-empty
/// list. (A *cycle* would not do: its closing edge is a return edge, so it does build a
/// chain — four links back to the root.)
#[test]
fn a_tree_leaves_the_reference_and_side_tables_alone() {
    let edges: Vec<(u32, u32)> = (0..7u32).map(|i| (i, i + 1)).collect();
    let adjacency = Adjacency::simple(8, &edges);
    let mut lr = Lr::new(&adjacency);
    lr.orient_all();
    lr.order_by_nesting_depth();
    assert!(lr.test_all(), "a path is planar");
    assert_eq!(links(&lr.reference), Vec::new(), "nothing to chain");
    assert_eq!(
        left_of_the_middle(&lr.side),
        Vec::<usize>::new(),
        "no side resolved"
    );
    assert!(lr.stack.is_empty());
}

/// Phase 2 re-derives its walk state from scratch: `ind` and `skip_init` are shared with
/// phase 1's DFS, which leaves both fully advanced, so `test_all` must clear them or the
/// second root (here a second, disjoint copy of the same component) is never visited at
/// all. Read through the output rather than through the flags: a component never walked
/// contributes no `reference` links of its own, so the second half of the chain comes
/// back empty.
#[test]
fn every_root_is_tested_not_just_the_first() {
    let edges = apexed_path(3).1;
    let joined: Vec<(u32, u32)> = edges
        .iter()
        .copied()
        .chain(edges.iter().map(|&(a, b)| (a + 5, b + 5)))
        .collect();
    let adjacency = Adjacency::simple(10, &joined);
    let mut lr = Lr::new(&adjacency);
    lr.orient_all();
    assert_eq!(lr.roots, [0, 5], "one root per component, ascending");
    lr.order_by_nesting_depth();
    assert!(lr.test_all(), "two components, both planar");
    assert_eq!(lr.roots, [0, 5], "one root per component, ascending");
    // Eight edges per component, so the second one's slots are 16 and up. Its phase-2
    // output must be the first one's links, shifted by 16.
    let shifted: Vec<(usize, u32)> = links(&lr.reference)
        .into_iter()
        .filter(|&(slot, _)| slot >= 16)
        .map(|(slot, target)| (slot - 16, target - 16))
        .collect();
    assert_eq!(shifted, vec![(4, 10), (8, 11), (9, 14), (13, 10)]);
    assert_eq!(left_of_the_middle(&lr.side), vec![11, 27]);
}

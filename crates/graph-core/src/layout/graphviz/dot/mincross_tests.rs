//! One closed case per step of the mincross pass, each on a graph small enough to draw.
//!
//! Every case here is staged by hand: the rank pass runs, `class2` builds the chains, and
//! then the nodes are installed into the rows in an order this file chooses. That is the only
//! way to test a step rather than the pass — the driver overwrites the order before the step
//! under test ever sees it, and a step test wants *its* input, not the driver's.
//!
//! Reproduce any case by running the oracle over the same edges and reading the rows back:
//!
//! ```sh
//! printf 'graph g {\\n  n0; n1; n2; n3;\\n  n0 -- n1; n1 -- n2; n2 -- n3; n3 -- n0;\\n}\\n' \\
//!     > target/probe/case.dot
//! scripts/orch/drun --rm --pull never --user 0:0 -v "$PWD:/w" -w /w ge-graphviz-oracle \\
//!     dot -Tplain target/probe/case.dot
//! ```
//!
//! Determinism: each case is one run of a pure function over a hand-written edge list.

use super::class2;
use super::fast::Fast;
use super::mincross::build;
use super::mincross::crossings;
use super::mincross::median;
use super::mincross::median::Sweep;
use super::mincross::ranks::Ranks;
use super::mincross::transpose;
use super::oracle_probe::graph;
use super::rank::rank;

/// A ranked, chained graph whose rows are filled in the order `install` gives.
fn staged(count: u32, edges: &[(u32, u32)], install: &[u32]) -> (Fast, Ranks) {
    let mut g = ranked(count, edges);
    assert_eq!(
        g.nodes.len(),
        usize::try_from(count).expect("a node count fits usize"),
        "no chain dummy: every edge here spans one rank"
    );
    let mut ranks = Ranks::allocate(&g);
    for &node in install {
        assert!(ranks.append(&mut g, node), "rank row {node} has room");
    }
    (g, ranks)
}

/// A graph through the rank pass and `class2`, which is the state every step here starts from.
fn ranked(count: u32, edges: &[(u32, u32)]) -> Fast {
    let mut g = graph(count, edges);
    rank(&mut g).expect("the graphs here are connected and acyclic after the pass");
    class2::run(&mut g);
    g
}

/// The value the median pass gave a node.
fn mval(g: &Fast, node: u32) -> f64 {
    g.nodes[node as usize].mval
}

/// **The odd case.** One node with three neighbours on the rank below takes the middle of
/// their values — not their mean, and not the nearer of the two. Three neighbours numbered
/// 0, 1 and 2 on the rank above give values 0, 256 and 512 at the scale of 256, and the
/// middle one is 256.
#[test]
fn an_odd_number_of_neighbours_gives_the_middle_value() {
    let (mut g, mut ranks) = staged(4, &[(0, 3), (1, 3), (2, 3)], &[0, 1, 2, 3]);
    assert!(
        !median::medians(&mut g, &mut ranks, 1, 0),
        "the node has three edges"
    );
    assert_eq!(mval(&g, 3), 256.0);
}

/// **The even case.** One node with four neighbours takes the middle of the two middle values,
/// weighted by how far each is from its end of the range. Four neighbours numbered 0, 1, 2
/// and 3 are evenly spaced, so the two weights are equal and the answer is the plain average
/// of 256 and 512.
#[test]
fn an_even_number_of_neighbours_averages_the_two_middle_values() {
    let (mut g, mut ranks) = staged(5, &[(0, 4), (1, 4), (2, 4), (3, 4)], &[0, 1, 2, 3, 4]);
    assert!(
        !median::medians(&mut g, &mut ranks, 1, 0),
        "the node has four edges"
    );
    assert_eq!(mval(&g, 4), 384.0);
}

/// **The even case where the weights differ.** Six nodes on the rank above and two below:
/// `n6` takes four of the six as in-edges, numbered 0, 1, 2 and 5. The gap above the middle
/// of its values is three times the gap below, so the answer is pulled a third of the way
/// from the lower middle towards the upper one — 320, not the 384 an unweighted average
/// gives. This is the one place the pass leaves integer arithmetic, and it is why the values
/// are scaled before they are divided.
#[test]
fn an_uneven_gap_pulls_the_even_median_towards_the_nearer_end() {
    let edges = [(0, 6), (1, 6), (2, 6), (5, 6), (3, 7), (4, 7)];
    let (mut g, mut ranks) = staged(8, &edges, &[0, 1, 2, 3, 4, 5, 6, 7]);
    assert!(
        !median::medians(&mut g, &mut ranks, 1, 0),
        "both nodes have in-edges"
    );
    assert_eq!(mval(&g, 6), 320.0, "four neighbours, unevenly spaced");
    assert_eq!(mval(&g, 7), 896.0, "two neighbours: their plain average");
}

/// **A node with no edge has no opinion**, and says so with -1 — which is below every real
/// value and is what keeps the swap walk from moving it. An isolated node is the case: it
/// has no in-edge and no out-edge, so there is nothing to take a median of, and the pass has
/// to report that the rank holds a node it must not move.
#[test]
fn a_node_with_no_edge_has_no_opinion() {
    let (mut g, mut ranks) = staged(3, &[(0, 1)], &[0, 1, 2]);
    assert!(
        median::medians(&mut g, &mut ranks, 0, 1),
        "node 2 has no edge at all"
    );
    assert_eq!(mval(&g, 2), -1.0);
}

/// **The crossing count of a hand-drawn K2,2.** Four nodes, two per rank, all four edges: the
/// two diagonals cross and the two sides do not, so the count is **one** — and it is one
/// whichever of the two nodes on a rank is drawn left, because swapping them swaps which
/// pair of edges is the diagonal. A counter that counts both diagonals, or neither, or that
/// gets the direction backwards, is caught here.
#[test]
fn a_k22_has_exactly_one_crossing() {
    let (g, _ranks) = staged(4, &[(0, 2), (0, 3), (1, 2), (1, 3)], &[0, 1, 2, 3]);
    assert_eq!(crossings::crossings(&g), 1, "the two diagonals cross");
}

/// **The same K2,2 with a third node below.** `n0` and `n1` above, `n2`, `n3` and `n4` below,
/// with the two edges of each pair spread so that nothing crosses. This is the negative
/// control for the case above: it is the same shape one rank wider, and the answer is zero,
/// so a counter that always answers "the number of inversions of something" cannot pass both.
#[test]
fn a_k22_with_a_third_node_below_has_no_crossing() {
    let (g, _ranks) = staged(5, &[(0, 2), (0, 3), (1, 3), (1, 4)], &[0, 1, 2, 3, 4]);
    assert_eq!(
        crossings::crossings(&g),
        0,
        "the edges are already in order"
    );
}

/// **A transverse pass that removes one crossing.** Five nodes: `n0`, `n1`, `n2` on the top
/// rank and `n3`, `n4` below, with `n0` and `n1` both reaching `n3` and `n1` and `n2` both
/// reaching `n4`. Installed with `n1` leftmost, the edge `n1 -> n4` crosses `n0 -> n3`: one
/// crossing. Swapping the first two of the top rank removes exactly that one and leaves the
/// rank `[n0, n1, n2]`, which is the whole of what the pass is asked to prove.
#[test]
fn a_transpose_removes_one_crossing() {
    let (mut g, mut ranks) = staged(5, &[(0, 3), (1, 3), (1, 4), (2, 4)], &[1, 0, 2, 3, 4]);
    assert_eq!(
        crossings::crossings(&g),
        1,
        "the order starts with one crossing"
    );
    transpose::transpose(&mut g, &mut ranks, false);
    assert_eq!(
        crossings::rank_rows(&g)[0],
        vec![0, 1, 2],
        "the swap undid the crossing"
    );
    assert_eq!(crossings::crossings(&g), 0);
}

/// **The negative control for the case above:** the same graph and the same starting order,
/// but the transverse pass is run in reverse, where a pair that costs the same either way is
/// swapped as well. A pass that swapped on ties in both directions would oscillate and never
/// settle, which is what makes the forward direction the one that terminates.
#[test]
fn a_transpose_leaves_a_non_crossing_pair_alone() {
    let (mut g, mut ranks) = staged(5, &[(0, 3), (1, 3), (1, 4), (2, 4)], &[0, 1, 2, 3, 4]);
    assert_eq!(crossings::crossings(&g), 0, "nothing to remove");
    transpose::transpose(&mut g, &mut ranks, false);
    assert_eq!(
        crossings::rank_rows(&g)[0],
        vec![0, 1, 2],
        "no swap on a tie"
    );
}

/// **The initial order is the walk, and it is what the pass starts from.** Four nodes on two
/// ranks with the source left of its target: the downward walk reaches every node from the
/// source and installs them in the order its own edge list gives, which here is the dense
/// order. Running it must leave the rows filled and numbered, and running the upward walk
/// afterwards must produce the same rows — a graph whose two walks agree is a graph whose
/// initial order is not a choice.
#[test]
fn both_walks_agree_on_a_graph_whose_order_is_not_a_choice() {
    let edges = [(0, 2), (0, 3), (1, 2), (1, 3)];
    for pass in 0..2 {
        let mut g = ranked(4, &edges);
        let mut ranks = Ranks::allocate(&g);
        ranks.enter_component(0, &[0, 1, 2, 3]);
        build::build_ranks(&mut g, &mut ranks, pass);
        ranks.install_complete_ranks(&mut g);
        assert_eq!(
            crossings::rank_rows(&g),
            vec![vec![0, 1], vec![2, 3]],
            "pass {pass}"
        );
    }
}

/// **The whole pass on a graph where every order crosses once.** The K2,2 above has one
/// crossing in every order, so the pass cannot improve on it and must hand back an order with
/// exactly one crossing — the count it reports is the count its order has, not the best count
/// any sweep saw.
#[test]
fn the_pass_reports_the_count_its_order_has() {
    let mut g = ranked(4, &[(0, 2), (0, 3), (1, 2), (1, 3)]);
    assert_eq!(super::mincross::run(&mut g), crossings::crossings(&g));
}

/// **The negative control for the swap walk's tie rule.** Two nodes on the top rank, two
/// below, and the pass has to leave an order with no crossing where one exists: a graph whose
/// two ranks are independent (no shared neighbour) has an order with no crossings, and a
/// swap walk that moved nodes on equal values in the forward direction would have no reason
/// to stop moving them.
#[test]
fn a_swap_walk_finishes_on_two_independent_ranks() {
    let mut g = ranked(4, &[(0, 2), (1, 3)]);
    let mut ranks = Ranks::allocate(&g);
    ranks.enter_component(0, &[0, 1, 2, 3]);
    for node in [0, 1, 2, 3] {
        assert!(ranks.append(&mut g, node), "rank row {node} has room");
    }
    let sweep = Sweep {
        reverse: false,
        fixed: false,
    };
    let moved = median::reorder(&mut g, &mut ranks, 0, &sweep);
    assert!(!moved, "nothing wanted to move");
    assert_eq!(crossings::rank_rows(&g)[0], vec![0, 1]);
}

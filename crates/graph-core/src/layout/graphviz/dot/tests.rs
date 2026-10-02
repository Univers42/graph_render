//! The ported passes' tests: cycle breaking and decomposition, checked against what the
//! reference's own code says they do rather than against a coordinate, because neither
//! pass has one yet.
//!
//! The closed *coordinates* are `docs/measurements/p13-gv2-dot.md`'s subject, and the
//! oracle prints them in `docs/measurements/p13-gv2-dot.md`'s closed-case table; what
//! these tests pin is the part of the pipeline that decides a graph's *shape*, which is
//! what a later pass inherits.

use super::fast::{Fast, Kind};
use super::{add_edges, break_cycles, empty_graph};

/// A graph with `count` nodes and `edges`, every node on Graphviz's default box.
fn graph(count: u32, edges: &[(u32, u32)]) -> Fast {
    let mut g = empty_graph(count);
    add_edges(&mut g, edges);
    g
}

/// The direction of every edge still in the fast graph, in dense-edge order: `(tail, head)`.
///
/// "Still in the fast graph" is what `Edge::live` means: `class1` copies each input edge in,
/// and the input records themselves are not in it. Reading the input records instead would
/// report edges the ranking pass never sees.
fn directions(g: &Fast) -> Vec<(u32, u32)> {
    g.edges
        .iter()
        .filter(|e| e.live)
        .map(|e| (e.tail, e.head))
        .collect()
}

/// The one live edge from `tail` to `head`.
fn live_edge(g: &Fast, tail: u32, head: u32) -> u32 {
    let found: Vec<u32> = g
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.live && e.tail == tail && e.head == head)
        .map(|(i, _)| i as u32)
        .collect();
    assert_eq!(
        found.len(),
        1,
        "exactly one live {tail} -> {head} in {found:?}"
    );
    found[0]
}

/// A graph with no cycles is left exactly as it was: the pass reads every edge and finds
/// no back edge, so no edge is touched.
#[test]
fn an_acyclic_graph_is_left_alone() {
    let mut g = graph(4, &[(0, 1), (1, 2), (2, 3)]);
    break_cycles(&mut g);
    assert_eq!(directions(&g), vec![(0, 1), (1, 2), (2, 3)]);
}

/// A two-cycle collapses to **one** edge, not two. This is `reverse_edge`
/// (`acyclic.c:22-33`) and it is the least obvious thing in the pass: the reversed edge
/// is unhooked, and because an edge the other way round already exists the two are
/// *merged*, so the survivor carries both weights. A port that swapped the endpoints
/// instead would keep two edges and get a different drawing.
#[test]
fn a_two_cycle_collapses_to_one_merged_edge() {
    let mut g = graph(2, &[(0, 1), (1, 0)]);
    break_cycles(&mut g);
    assert_eq!(directions(&g), vec![(0, 1)]);
    let survivor = live_edge(&g, 0, 1);
    assert_eq!(
        g.edges[survivor as usize].weight, 2,
        "the two declared weights add"
    );
    assert_eq!(g.edges[survivor as usize].count, 2, "and so do the counts");
}

/// A three-cycle `n0 -> n1 -> n2 -> n0`: the search from `n0` walks to `n2` and finds
/// `n2 -> n0` on the path, so the *last* edge of the cycle is the one reversed.
#[test]
fn a_three_cycle_reverses_its_closing_edge() {
    let mut g = graph(3, &[(0, 1), (1, 2), (2, 0)]);
    break_cycles(&mut g);
    assert_eq!(directions(&g), vec![(0, 1), (1, 2), (0, 2)]);
}

/// The pass is a pure function of the graph: two runs agree edge for edge, which is the
/// property the rest of the pipeline inherits and the reason no pass may read a clock.
#[test]
fn two_runs_agree_edge_for_edge() {
    let edges = [(0, 1), (1, 2), (2, 3), (3, 0), (2, 0), (0, 3), (3, 1)];
    let mut first = graph(5, &edges);
    let mut second = graph(5, &edges);
    break_cycles(&mut first);
    break_cycles(&mut second);
    assert_eq!(directions(&first), directions(&second));
}

/// After the pass every edge runs strictly downwards in a rank assignment, which is what
/// makes a layering possible at all. Checked by asking for one: no edge's head is
/// reachable from its tail.
#[test]
fn no_cycle_survives() {
    let mut g = graph(
        6,
        &[
            (0, 1),
            (1, 2),
            (2, 3),
            (3, 4),
            (4, 5),
            (5, 0),
            (0, 5),
            (2, 5),
        ],
    );
    break_cycles(&mut g);
    assert!(is_acyclic(&g), "{:?}", directions(&g));
}

/// Is there a directed cycle? An ordinary depth-first search with three colours.
fn is_acyclic(g: &Fast) -> bool {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        White,
        Grey,
        Black,
    }
    fn walk(g: &Fast, node: u32, mark: &mut [Mark]) -> bool {
        mark[node as usize] = Mark::Grey;
        for &edge in &g.out[node as usize] {
            let e = &g.edges[edge as usize];
            if !e.live {
                continue;
            }
            match mark[e.head as usize] {
                Mark::Grey => return false,
                Mark::White => {
                    if !walk(g, e.head, mark) {
                        return false;
                    }
                }
                Mark::Black => {}
            }
        }
        mark[node as usize] = Mark::Black;
        true
    }
    let mut mark = vec![Mark::White; g.nodes.len()];
    (0..g.nodes.len() as u32).all(|n| mark[n as usize] != Mark::Grey && walk(g, n, &mut mark))
}

/// The components of a connected graph are one component holding every node, in the
/// reference's pop order. `class1` runs first because `decompose` searches the fast graph,
/// which is empty until `class1` fills it.
#[test]
fn a_connected_graph_is_one_component() {
    let mut g = graph(5, &[(0, 1), (1, 2), (2, 3), (3, 4)]);
    super::class1::run(&mut g);
    let comps = super::decomp::decompose(&g);
    assert_eq!(comps.len(), 1);
    assert_eq!(comps[0].len(), 5);
}

/// The components are found in the order their first node appears in, which is the dense
/// node order, and an isolated node is a component of its own — the reference seeds the
/// search from every real node, connected or not (`decomp.c:91-113`).
#[test]
fn components_come_out_in_dense_index_order() {
    let mut g = graph(6, &[(0, 1), (3, 4), (1, 2)]);
    super::class1::run(&mut g);
    let comps = super::decomp::decompose(&g);
    assert_eq!(comps.len(), 3, "{comps:?}");
    assert_eq!(comps[0], vec![0, 1, 2]);
    assert_eq!(comps[1], vec![3, 4]);
    assert_eq!(comps[2], vec![5], "an isolated node is its own component");
}

/// Reversing an edge that already has a reverse **merges** the two, which is the
/// reference's `reverse_edge` (`acyclic.c:22-33`) and not a swap: the surviving edge
/// carries both weights. Both directions are live here because `class1` folds *parallel*
/// edges together and a pair running the other way round is not parallel — it is exactly the
/// case `acyclic` exists to resolve.
#[test]
fn reversing_onto_an_existing_reverse_merges_the_two() {
    let mut g = graph(2, &[(0, 1), (1, 0)]);
    super::class1::run(&mut g);
    assert_eq!(directions(&g), vec![(0, 1), (1, 0)]);
    let reversed = live_edge(&g, 1, 0);
    let first = g.edges[reversed as usize].weight;
    g.reverse_edge(reversed);
    assert!(
        !g.edges[reversed as usize].live,
        "the reversed edge is unhooked"
    );
    let survivor = live_edge(&g, 0, 1);
    assert_eq!(
        g.edges[survivor as usize].weight,
        first * 2,
        "the weights add"
    );
}

/// `class1` gives each input edge a copy in the fast graph and leaves the input record out
/// of it. That is not bookkeeping: `find_fast_edge` asks whether a pair already has an edge,
/// and the answer has to be "no" for the edge being examined — an input edge moved in rather
/// than copied would answer "yes" and make `merge_oneway` merge an edge into itself.
#[test]
fn class1_copies_input_edges_into_the_fast_graph() {
    let mut g = graph(3, &[(0, 1), (1, 2)]);
    assert!(
        g.edges.iter().all(|e| !e.live),
        "an input edge is not in the fast graph until class1 runs"
    );
    super::class1::run(&mut g);
    assert_eq!(directions(&g), vec![(0, 1), (1, 2)]);
    let live: Vec<usize> = g
        .edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.live)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(live, vec![2, 3], "the copies follow the two input records");
}

/// Two input edges between the same pair are one constraint with double the weight, before
/// ranking rather than after: `class1`'s `find_fast_edge` finds the copy the first one made.
#[test]
fn class1_folds_parallel_input_edges_together() {
    let mut g = graph(2, &[(0, 1), (0, 1)]);
    super::class1::run(&mut g);
    assert_eq!(directions(&g), vec![(0, 1)], "one constraint, not two");
    let survivor = live_edge(&g, 0, 1);
    assert_eq!(g.edges[survivor as usize].weight, 2);
    assert_eq!(g.edges[survivor as usize].count, 2);
}

/// A virtual edge is the reference's own: a one-point box widened by `nodesep / 2` on each
/// side (`fastgr.c:200-213` and `class2.c`'s `incr_width`).
#[test]
fn a_virtual_node_is_a_nodesep_wide_dummy() {
    let node = Kind::Virtual;
    assert_eq!(node, Kind::Virtual);
    let n = crate::layout::graphviz::dot::fast::Node::virtual_node(super::NODESEP);
    assert_eq!(n.kind, Kind::Virtual);
    assert_eq!(n.lw, 1.0 + (super::NODESEP / 2.0).trunc());
    assert_eq!(n.ht, 1.0);
}

/// Every real node starts on Graphviz's default box, which is what the closed coordinates
/// in the measurements file are stated against.
#[test]
fn every_node_starts_on_the_default_box() {
    let g = empty_graph(3);
    for node in &g.nodes {
        assert_eq!(node.lw, 0.75 * 72.0 / 2.0);
        assert_eq!(node.rw, 0.75 * 72.0 / 2.0);
        assert_eq!(node.ht, 0.5 * 72.0);
        assert_eq!(node.kind, Kind::Normal);
    }
}

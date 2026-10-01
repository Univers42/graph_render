//! The two halves of the port that are not the drawing: `_disk_positions`' rounding
//! (`hierarchical.py:90-111`) and `_multi_source_levels`' re-seed (`:75-80`).
//!
//! Split from the parent's own tests, which are about the drawing, because these are about
//! arithmetic and traversal — the two things a hand-plotted coordinate cannot falsify on its
//! own. `half_to_even` in particular is the one rule in the whole port where a plausible
//! delegation (`f64::round`) is not a small slip but a different drawing, so it is pinned
//! against its own naive control here rather than as an aside.
//!
//! The helpers the parent's tests also use (`build`, `nodes`, `edge`, `simple_graph`) come
//! from `super`, so there is one spelling of "a topology of ids `a`, `b`, `c`" in the module.

use super::super::{ID, run};
use super::{build, edge, nodes, space};
use crate::layout::force::simple_graph;
use crate::layout::hierarchical_3d::disk::{half_to_even, ring_take};
use crate::layout::hierarchical_3d::levels::multi_source_levels;

/// The rounding rule itself, with the naive `f64::round` as its own negative control:
/// the two disagree at **every** exact half, so a port that delegated would not be a
/// small-slip port, it would be a different drawing.
#[test]
fn half_to_even_rounds_a_tie_to_even_where_f64_round_rounds_away() {
    for (value, want) in [
        (0.5_f64, 0.0_f64),
        (1.5, 2.0),
        (2.5, 2.0),
        (3.5, 4.0),
        (7.5, 8.0),
        (-0.5, 0.0),
        (-1.5, -2.0),
        (2.4, 2.0),
        (2.6, 3.0),
    ] {
        assert_eq!(half_to_even(value), want, "half_to_even({value})");
    }
    assert_eq!(
        2.5_f64.round(),
        3.0,
        "the control: f64::round is half AWAY from zero"
    );
    assert_ne!(
        half_to_even(2.5),
        2.5_f64.round(),
        "so a naive round is not this rule"
    );
}

/// `take` and both fix-up loops (`hierarchical.py:99-103`), each pinned on a case that
/// exercises one of the three lines. `count = 23` overshoots (`[3, 8, 13]`, sum 24) and the
/// `while take.sum() > count` loop takes one unit off the argmax; `count = 22` undershoots
/// (`[2, 7, 12]`, sum 21) and `while take.sum() < count` gives one to `take[-1]`.
#[test]
fn ring_take_lands_on_count_by_both_fix_up_loops() {
    assert_eq!(
        ring_take(10, 2),
        vec![2, 8],
        "ties to even, no fix-up needed"
    );
    assert_eq!(
        ring_take(23, 3),
        vec![3, 8, 12],
        "sum > count: argmax loses one"
    );
    assert_eq!(
        ring_take(22, 3),
        vec![2, 7, 13],
        "sum < count: take[-1] gains one"
    );
    for count in 2..200usize {
        let rings = super::super::disk::ring_count(count);
        let total: i64 = ring_take(count, rings).iter().sum();
        assert_eq!(total as usize, count, "count {count} over {rings} rings");
    }
}

/// `_multi_source_levels`'s re-seed (`hierarchical.py:75-80`): a node no root reaches
/// seeds a level zero of its own. Handed `roots = [0]` on two disjoint pairs, the second
/// pair is reached by no root, so node 2 seeds level 0 and node 3 hangs off it.
///
/// Unreachable through `run`, and that is worth saying plainly: `_component_roots`
/// returns one root per component, so under the undirected branch this port takes, every
/// node is reachable and the loop below never fires. It is ported because the reference
/// shares the function with its directed branch, which this module does not take.
#[test]
fn a_node_no_root_reaches_seeds_a_level_zero_of_its_own() {
    let ids = ["a", "b", "c", "d"];
    let topology = build(&nodes(&ids), &[edge("ab", "a", "b"), edge("cd", "c", "d")]);
    let graph = simple_graph(&topology);
    let got = multi_source_levels(&graph, &[0], topology.node_count());
    assert_eq!(got, vec![(0, 0), (1, 1), (2, 0), (3, 1)]);
    assert_eq!(got[2].1, 0, "c is its own root");
}

/// The graph the re-seed exists for, in the shape a directed branch would hand it: a
/// 2-cycle has no in-degree-0 node at all. Read through the *undirected* projection
/// `run` takes, the two arcs are one unordered pair, so `simple_graph` collapses them and
/// the drawing is the two-node case — pinned here so the collapse is a test, not a claim.
#[test]
fn a_two_cycle_collapses_to_one_undirected_edge() {
    let ids = ["a", "b"];
    let topology = build(&nodes(&ids), &[edge("ab", "a", "b"), edge("ba", "b", "a")]);
    assert_eq!(topology.edge_count(), 2, "the topology keeps both arcs");
    assert_eq!(
        simple_graph(&topology).lo,
        vec![0],
        "the projection keeps one"
    );
    let (x, y, z) = space(&run(&topology).expect("fits"));
    // The root is **b**, not `a`, and that asymmetry is the reference's own on a two-node
    // graph: the first sweep from `a` ends at `b`, the second from `b` ends at `a`, the
    // chain is rebuilt from `far_order[-1] = a`, so `path = [a, b]` and
    // `path[len(path) // 2] = path[1] = b` (`hierarchical.py:48-51`). Level 0 is `b`,
    // level 1 is `a`, `max_level = 1`, so z is `-5` and `+5`.
    assert_eq!((z[1], z[0]), (-5.0_f32, 5.0), "root b, a one level out");
    // Both levels hold exactly one node, so both take `_disk_positions`' `count == 1` line
    // (`hierarchical.py:95-96`) and sit on their own axis whatever radius was computed.
    assert_eq!(
        (x[0], y[0], x[1], y[1]),
        (0.0_f32, 0.0_f32, 0.0_f32, 0.0_f32),
        "one node per level is the count-1 disk case twice over"
    );
}

#[test]
fn the_capability_id_is_the_one_the_ledger_will_list() {
    assert_eq!(ID, "layout.hierarchical3d");
}

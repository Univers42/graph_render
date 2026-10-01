//! `layout.circular.hierarchy` against SciGraphs' own `_circular_hierarchy_layout`.
//!
//! Every `want` below is the reference's printed answer, read out of
//! `SciGraphs/core/scigraphs_core/mesh/layouts/hierarchical.py:693-732` run in the
//! `ge-python-oracle` image at `scale = 5.0` (the dispatcher's own default) over the same
//! `nx.Graph` the fixture builds — undirected, self-loops dropped, a repeated unordered
//! pair merged. The comparison is a tolerance because the port narrows to `f32` at the end
//! and `np.cos` is not `libm::cos` bit for bit; `harness/oracle-circular-hierarchy.py` is
//! the 1000-seed arm of the same question.

use super::{ID, run};
use crate::layout::coords::probe::{assert_close, graph, points};

#[test]
fn the_id_names_this_layout_and_nothing_else() {
    assert_eq!(ID, "layout.circular.hierarchy");
}

#[test]
fn an_empty_graph_has_no_points() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
}

#[test]
fn one_node_is_a_lone_level_zero_and_sits_on_the_axis() {
    assert_eq!(points(&run(&graph(1, &[])).unwrap()), vec![(0.0, 0.0)]);
}

/// A path's double-BFS root is its exact middle, so the rings are `0, 2.5, 5` — the
/// `max(level, 0.35) * scale / max(2, max_level)` rule with `max_level = 2`.
#[test]
fn a_path_puts_its_middle_node_at_the_centre_and_its_ends_on_the_outer_ring() {
    let got = points(&run(&graph(5, &[(0, 1), (1, 2), (2, 3), (3, 4)])).unwrap());
    assert_close(
        &got,
        &[
            (5.0, 0.0),
            (2.5, 0.0),
            (0.0, 0.0),
            (-2.5, 3.061617e-16),
            (-5.0, 6.123234e-16),
        ],
        1e-6,
    );
}

/// A star's root is the hub, and its leaves share one ring at `scale / 2` because
/// `max_level` is 1 and the divisor is `max(2, max_level)`.
#[test]
fn a_star_puts_the_hub_on_the_axis_and_the_leaves_on_one_ring() {
    let got = points(&run(&graph(5, &[(0, 1), (0, 2), (0, 3), (0, 4)])).unwrap());
    assert_close(
        &got,
        &[
            (0.0, 0.0),
            (2.5, 0.0),
            (1.5308085e-16, 2.5),
            (-2.5, 3.061617e-16),
            // The `sin(3pi/2)` residual the reference prints, at f64's own precision:
            // `-4.592_426e-16` is `-4.5924255e-16` truncated, which is the same double.
            (-4.592_426e-16, -2.5),
        ],
        1e-6,
    );
}

/// A cycle's root is the far end of the second sweep, not node 0, and level 1 is filled
/// in CSR adjacency order — which is why node 1 takes angle 0 and node 0 takes angle pi.
#[test]
fn a_triangle_roots_at_its_own_far_end_not_at_node_zero() {
    let got = points(&run(&graph(3, &[(0, 1), (1, 2), (0, 2)])).unwrap());
    assert_close(&got, &[(-2.5, 3.061617e-16), (2.5, 0.0), (0.0, 0.0)], 1e-6);
}

/// Three isolated nodes are three one-node components, so one level of three: the `0.35`
/// floor at `scale / 2` is the whole radius rule here.
#[test]
fn three_isolated_nodes_share_the_small_root_ring() {
    let got = points(&run(&graph(3, &[])).unwrap());
    assert_close(
        &got,
        &[(0.875, 0.0), (-0.4375, 0.7577722), (-0.4375, -0.7577722)],
        1e-6,
    );
}

/// Two components: the double-BFS midpoint of a 2-node component is the *second* node, so
/// the roots are 1 and 3 and node 0 is a level-1 node, not a level-0 one.
#[test]
fn each_component_gets_its_own_root_and_the_levels_are_pooled() {
    let got = points(&run(&graph(4, &[(0, 1), (2, 3)])).unwrap());
    assert_close(
        &got,
        &[
            (2.5, 0.0),
            (0.875, 0.0),
            (-2.5, 3.061617e-16),
            (-0.875, 1.0715659e-16),
        ],
        1e-6,
    );
}

/// A self-loop and a repeated unordered pair are the same graph to both arms: `nx.Graph`
/// merges them silently and `simple_graph` drops them, so the answer is the plain path's.
#[test]
fn a_self_loop_and_a_repeated_pair_are_the_simple_graph() {
    let noisy = points(&run(&graph(3, &[(0, 0), (0, 1), (1, 2), (0, 1)])).unwrap());
    let clean = points(&run(&graph(3, &[(0, 1), (1, 2)])).unwrap());
    assert_eq!(noisy, clean);
    assert_close(
        &noisy,
        &[(2.5, 0.0), (0.0, 0.0), (-2.5, 3.061617e-16)],
        1e-6,
    );
}

/// A single node with a self-loop only: one component, one level, one node on the axis.
#[test]
fn a_lone_self_loop_is_still_a_lone_node_on_the_axis() {
    let got = points(&run(&graph(1, &[(0, 0)])).unwrap());
    assert_eq!(got, vec![(0.0, 0.0)]);
}

#[test]
fn two_runs_are_bit_identical() {
    let t = graph(
        9,
        &[
            (0, 1),
            (1, 2),
            (3, 4),
            (4, 5),
            (5, 6),
            (6, 7),
            (7, 8),
            (8, 0),
        ],
    );
    let once = run(&t).unwrap();
    let twice = run(&t).unwrap();
    assert_eq!(points(&once), points(&twice));
}

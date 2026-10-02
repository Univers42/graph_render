//! The module's own glue: the one reduction both paths share, and the branch that chooses
//! between them. Everything here is about *which* graph the two paths get and *which* one
//! runs — the arithmetic itself is pinned in each submodule.

use super::super::{loop_counts, simple_pairs};
use crate::index::{Topology, index_model};
use crate::layout::circle_packing::CirclePackingParams;
use crate::layout::circle_packing::tests::support::{complete_graph, grid, topology, wheel};

/// The reduced edge list, as dense-index pairs, in the order the topology holds them.
fn reduced(n: u32, edges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    simple_pairs(&topology(n, edges))
}

/// The per-node self-loop count the fallback's starting degree needs.
fn loops(n: u32, edges: &[(u32, u32)]) -> Vec<u32> {
    loop_counts(&topology(n, edges), n)
}

#[test]
fn loop_counts_are_per_node_and_skip_every_other_pair() {
    // The two lists the fallback is handed are independent: the reduction drops the loops,
    // and this is what puts them back for the degree networkx reads them by.
    // `(0, 0)` twice is one loop: `nx.Graph` (`common.py:238`) stores a repeated edge once,
    // so networkx's degree for node 0 is `len({1, 0}) + 1 = 3`, never 5.
    let edges = [(0, 1), (0, 0), (0, 0), (1, 2), (3, 3)];
    assert_eq!(loops(4, &edges), vec![1, 0, 0, 1]);
    assert_eq!(
        reduced(4, &edges),
        vec![(0, 1), (1, 2)],
        "the loops are gone"
    );
    assert_eq!(
        loops(3, &[(0, 1), (1, 2)]),
        vec![0, 0, 0],
        "no loops at all"
    );
    assert_eq!(loops(2, &[]), vec![0, 0], "not even an empty one");
    // A loop is a self-edge whichever end order the record spells it, and it lands in the
    // slot of the node it names — one below a higher-numbered loop keeps its own index.
    assert_eq!(loops(5, &[(4, 4), (1, 1)]), vec![0, 1, 0, 0, 1]);
}

#[test]
fn a_plain_simple_graph_is_left_exactly_as_it_is() {
    let edges = [(0, 1), (1, 2), (2, 0), (2, 3)];
    assert_eq!(reduced(4, &edges), edges, "nothing to reduce");
}

#[test]
fn a_duplicate_pair_is_kept_once_whichver_way_round_it_is_written() {
    // `(u, v)` and `(v, u)` are the same undirected edge, and the reduction keeps the
    // first occurrence, so the surviving pair is spelled the way the topology spelled it.
    assert_eq!(reduced(3, &[(0, 1), (1, 0), (1, 2)]), vec![(0, 1), (1, 2)]);
    assert_eq!(reduced(3, &[(0, 1), (0, 1), (0, 1)]), vec![(0, 1)]);
    assert_eq!(reduced(3, &[(1, 0), (0, 1)]), vec![(1, 0)]);
}

#[test]
fn a_self_loop_is_dropped() {
    // SciGraphs' own `if u != v` filter (`circle_packing.py:61`): a self-loop is not a
    // spring, and it is not a plane-graph edge either.
    assert_eq!(reduced(2, &[(0, 0), (0, 1)]), vec![(0, 1)]);
    assert_eq!(reduced(3, &[(1, 1), (0, 1), (2, 2)]), vec![(0, 1)]);
    assert!(reduced(2, &[(0, 0), (1, 1)]).is_empty(), "only self-loops");
}

#[test]
fn the_reduction_keeps_the_topologys_own_edge_order() {
    // The springs are gathered in this order and the sum order is part of the output's
    // bits (D5), so the reduction is by first occurrence, never by sorting.
    let edges = [(2, 3), (0, 1), (2, 3), (1, 0), (0, 2)];
    assert_eq!(reduced(4, &edges), vec![(2, 3), (0, 1), (0, 2)]);
}

#[test]
fn a_graph_with_no_edges_reduces_to_nothing() {
    assert!(reduced(4, &[]).is_empty());
    assert!(reduced(0, &[]).is_empty());
}

#[test]
fn the_reduction_reads_the_topologys_own_edge_count() {
    // A topology built by hand rather than by the test helper, so the edge count and the
    // edge list are the pipeline's own, not a fixture's.
    let t: Topology = index_model(
        &[
            crate::records::build::node("a", ""),
            crate::records::build::node("b", ""),
        ],
        &[
            crate::records::build::edge("e0", "a", "b"),
            crate::records::build::edge("e1", "b", "a"),
            crate::records::build::edge("e2", "a", "a"),
        ],
    )
    .expect("fits");
    assert_eq!(t.edge_count(), 3, "the topology keeps all three");
    assert_eq!(
        simple_pairs(&t),
        vec![(0, 1)],
        "the reduction is the layout's job"
    );
}

#[test]
fn a_planar_graph_takes_the_exact_path_and_a_non_planar_one_the_fallback() {
    let (n, edges) = wheel(6);
    let exact = crate::layout::circle_packing::run(&topology(n, &edges)).expect("runs");
    assert!(exact.notes.is_empty(), "a wheel is planar: the exact path");
    let non_planar =
        crate::layout::circle_packing::run(&topology(5, &complete_graph(5))).expect("runs");
    assert_eq!(non_planar.notes.len(), 1, "K5 is not: the fallback");
}

#[test]
fn a_sweep_budget_is_twenty_times_the_iterations_not_twenty_more() {
    // SciGraphs' own `max(int(iterations), 1) * 20` (`circle_packing.py:330`) is a
    // multiplication. A wheel of eight needs more than 20 sweeps and fewer than 40, so
    // the two readings of that line disagree about it: at `iterations = 2` the reference
    // runs 40 sweeps and certifies, while `iterations + 20` would run 22 and fall back.
    let (n, edges) = wheel(8);
    let at = |iterations| {
        crate::layout::circle_packing::run_with(
            &topology(n, &edges),
            &CirclePackingParams {
                iterations,
                scale: 5.0,
            },
        )
        .expect("runs")
    };
    let starved = at(1);
    assert_eq!(
        starved.notes.len(),
        1,
        "1 iteration is 20 sweeps, which is not enough: {:?}",
        starved.notes
    );
    let fed = at(2);
    assert!(
        fed.notes.is_empty(),
        "2 iterations is 40 sweeps, which converges: {:?}",
        fed.notes
    );
    // A zero budget is the same as one — the reference's own `max(int(iterations), 1)`,
    // so a caller passing 0 does not silently get an unpacked graph.
    assert_eq!(at(0), starved);
}

#[test]
fn a_scale_the_reference_would_never_produce_is_still_refused() {
    // `check_scale`'s own rule: finite and above zero, which is what every downstream
    // `scale * 0.45` and `sqrt(0.35 * frame^2)` needs to stay finite.
    for bad in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let params = CirclePackingParams {
            iterations: 10,
            scale: bad,
        };
        let err = crate::layout::circle_packing::run_with(&topology(3, &[(0, 1)]), &params)
            .expect_err("refused");
        assert_eq!(
            err.to_string(),
            "parameter scale: finite and above 0",
            "{bad}"
        );
    }
    // A tiny positive scale is accepted: it is finite and above 0, and every derived
    // quantity is correspondingly tiny rather than degenerate.
    let tiny = CirclePackingParams {
        iterations: 5,
        scale: 1e-30,
    };
    let (n, edges) = grid(3, 3);
    let geometry =
        crate::layout::circle_packing::run_with(&topology(n, &edges), &tiny).expect("runs");
    let graph_contract::geometry::NodeGeometry::Circle { x, y, r } = &geometry.nodes else {
        panic!("circle packing always emits Circle geometry");
    };
    // Centres may be on either side of the origin; only the radii must be non-negative,
    // and nothing may be NaN or infinite.
    assert!(x.iter().chain(y).all(|v| v.is_finite()), "x {x:?} y {y:?}");
    assert!(r.iter().all(|v| v.is_finite() && *v >= 0.0), "r {r:?}");
    // The packing still has the frame's shape at this scale: it is the same packing.
    let span = (x[n as usize - 1] - x[0]).abs();
    assert!(span > 0.0 && span < 1e-29, "span {span}");
}

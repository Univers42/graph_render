use super::partition::partition;
use super::run;
use crate::layout::adjacency::neighbours;
use crate::layout::coords::probe::{assert_close, graph, points};

#[test]
fn empty_graph_has_no_points_and_one_node_is_at_the_origin_column() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
    let one = points(&run(&graph(1, &[])).unwrap());
    assert_eq!(one, vec![(0.0, 0.0)]);
}

#[test]
fn a_path_matches_networkx_bipartite_layout() {
    let got = points(&run(&graph(4, &[(0, 1), (1, 2), (2, 3)])).unwrap());
    assert_close(
        &got,
        &[(-1.0, -0.75), (1.0, -0.75), (-1.0, 0.75), (1.0, 0.75)],
        1e-6,
    );
}

#[test]
fn an_isolated_node_joins_the_first_set_like_scigraphs() {
    let got = points(&run(&graph(5, &[(0, 1), (1, 2), (2, 3)])).unwrap());
    let want = [
        (-0.6666667, -0.625),
        (1.0, -0.625),
        (-0.6666667, 0.0),
        (1.0, 0.625),
        (-0.6666667, 0.625),
    ];
    assert_close(&got, &want, 1e-6);
}

#[test]
fn a_triangle_is_not_bipartite_and_splits_by_greedy_max_cut_without_panic() {
    let topology = graph(3, &[(0, 1), (1, 2), (2, 0)]);
    let (a, b) = partition(&neighbours(&topology));
    assert_eq!((a, b), (vec![0, 2], vec![1]));
    let got = points(&run(&topology).unwrap());
    assert_close(&got, &[(-0.5, -0.375), (1.0, -0.375), (-0.5, 0.75)], 1e-6);
}

#[test]
fn a_self_loop_makes_the_graph_non_bipartite_and_never_panics() {
    let got = points(&run(&graph(2, &[(0, 0), (0, 1)])).unwrap());
    assert_eq!(got.len(), 2);
    assert!(got.iter().all(|p| p.0.is_finite() && p.1.is_finite()));
}

#[test]
fn a_disconnected_bipartite_graph_balances_the_two_sets() {
    let topology = graph(5, &[(0, 1), (0, 2), (3, 4)]);
    let (a, b) = partition(&neighbours(&topology));
    assert_eq!((a, b), (vec![0, 3], vec![1, 2, 4]));
}

use super::run;
use crate::layout::coords::probe::{assert_close, graph, points};

#[test]
fn empty_graph_has_no_points() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
}

#[test]
fn one_node_sits_at_the_centre() {
    assert_eq!(points(&run(&graph(1, &[])).unwrap()), vec![(0.0, 0.0)]);
}

#[test]
fn two_nodes_are_opposite_on_the_unit_circle() {
    let got = points(&run(&graph(2, &[])).unwrap());
    assert_close(&got, &[(1.0, 0.0), (-1.0, 0.0)], 1e-6);
}

#[test]
fn five_nodes_match_networkx_circular_layout() {
    let got = points(&run(&graph(5, &[(0, 1)])).unwrap());
    let want = [
        (1.0, 0.0),
        (0.309017, 0.9510566),
        (-0.8090171, 0.5877853),
        (-0.809017, -0.5877853),
        (0.3090171, -0.9510565),
    ];
    assert_close(&got, &want, 1e-6);
}

#[test]
fn edges_do_not_move_nodes_and_a_disconnected_graph_still_lands_on_the_ring() {
    let a = points(&run(&graph(4, &[])).unwrap());
    let b = points(&run(&graph(4, &[(0, 1), (2, 3)])).unwrap());
    assert_eq!(a, b);
    assert_close(
        &a,
        &[(1.0, 0.0), (0.0, 1.0), (-1.0, 0.0), (0.0, -1.0)],
        1e-6,
    );
}

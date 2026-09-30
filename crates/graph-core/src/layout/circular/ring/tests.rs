use super::{run, run_under, run_with};
use crate::exec::Serial;
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

/// Every width writes the same ring: the gather is a per-node function and the merge below
/// it is a serial loop over the whole column, so the division of the outputs cannot move a
/// byte. Sizes chosen to divide unevenly by every width the gate runs.
#[test]
fn the_ring_is_the_same_bytes_at_every_worker_count() {
    for n in [2, 3, 5, 16, 17, 64, 1000] {
        let want = points(&run(&graph(n, &[])).unwrap());
        for workers in [1, 2, 3, 4, 7] {
            let got = run_with(&graph(n, &[]), &Serial, workers).unwrap();
            assert_eq!(points(&got), want, "n = {n}, workers = {workers}");
        }
    }
}

/// The negative control reaches the **merge** and not the gather: the angles are untouched
/// and the centroid is stolen, so a threaded arm that ran the merge is red.
#[test]
fn the_control_diverges_the_ring_from_the_honest_run() {
    let honest = points(&run(&graph(5, &[])).unwrap());
    let stolen = points(&run_under(&graph(5, &[]), &Serial, 3, true).unwrap());
    assert_ne!(honest, stolen, "the shared merge did not move");
}

use super::run;
use crate::layout::coords::probe::{graph, points};
use crate::synthetic::Mulberry32;

#[test]
fn empty_graph_has_no_points_and_one_node_is_in_the_unit_square() {
    assert_eq!(points(&run(&graph(0, &[])).unwrap()), vec![]);
    let one = points(&run(&graph(1, &[])).unwrap());
    assert_eq!(one.len(), 1);
    assert!((0.0..1.0).contains(&one[0].0) && (0.0..1.0).contains(&one[0].1));
}

#[test]
fn positions_are_the_seeded_stream_row_major() {
    let mut rng = Mulberry32::new(super::SEED);
    let want: Vec<(f32, f32)> = (0..3)
        .map(|_| (rng.next_f64() as f32, rng.next_f64() as f32))
        .collect();
    assert_eq!(points(&run(&graph(3, &[(0, 1)])).unwrap()), want);
}

#[test]
fn a_disconnected_graph_gets_the_same_points_as_an_edgeless_one() {
    let a = points(&run(&graph(4, &[])).unwrap());
    assert_eq!(a, points(&run(&graph(4, &[(0, 1), (2, 3)])).unwrap()));
    assert!(
        a.iter()
            .all(|p| (0.0..1.0).contains(&p.0) && (0.0..1.0).contains(&p.1))
    );
}

#[test]
fn the_first_pair_is_pinned() {
    let got = points(&run(&graph(2, &[])).unwrap());
    assert_eq!(got[0], (0.71003205, 0.28633666));
}

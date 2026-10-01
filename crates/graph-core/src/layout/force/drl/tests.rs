use super::{Drl, DrlParams};
use crate::index::{Topology, empty_model, index_model};
use crate::records::build::{edge, node};
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

fn graph(n: u32, pairs: &[(u32, u32)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

fn points(t: &Topology, params: &DrlParams) -> (Vec<f32>, Vec<f32>) {
    match Drl::run(t, params).expect("finite").nodes {
        NodeGeometry::Point { x, y } => (x, y),
        other => panic!("point geometry, got {other:?}"),
    }
}

fn short() -> DrlParams {
    let mut p = DrlParams::default();
    for phase in &mut p.phases {
        phase.iterations /= 10;
    }
    p
}

#[test]
fn a_path_of_four_has_pinned_coordinates() {
    let (x, y) = points(&graph(4, &[(0, 1), (1, 2), (2, 3)]), &DrlParams::default());
    let bits: Vec<u32> = x.iter().chain(&y).map(|v| v.to_bits()).collect();
    assert_eq!(
        bits,
        [
            3240335041, 3239765452, 3239154819, 3238597030, 3227527194, 3227939588, 3227973277,
            3227288794
        ]
    );
}

#[test]
fn empty_and_single_node_graphs_work() {
    let (x, _) = points(&empty_model(), &DrlParams::default());
    assert!(x.is_empty());
    let (x, y) = points(&graph(1, &[]), &short());
    assert_eq!((x, y), (vec![0.0], vec![0.0]));
}

#[test]
fn a_disconnected_graph_stays_finite_and_in_the_plane() {
    let t = graph(6, &[(0, 1), (1, 2), (3, 4)]);
    let (x, y) = points(&t, &short());
    assert!(
        x.iter()
            .chain(&y)
            .all(|v| v.is_finite() && v.abs() < 2000.0)
    );
}

#[test]
fn an_isolated_node_never_moves() {
    let (x, y) = points(&graph(4, &[(0, 1), (1, 2)]), &short());
    assert_eq!((x[3], y[3]), (0.0, 0.0));
}

#[test]
fn the_same_seed_repeats_and_another_differs() {
    let t = graph(6, &[(0, 1), (0, 2), (1, 3), (2, 4), (4, 5)]);
    assert_eq!(points(&t, &short()), points(&t, &short()));
    let other = DrlParams { seed: 9, ..short() };
    assert_ne!(points(&t, &short()), points(&t, &other));
}

#[test]
fn neighbours_end_closer_than_strangers() {
    let t = graph(6, &[(0, 1), (1, 2), (0, 2), (3, 4), (4, 5), (3, 5)]);
    let (x, y) = points(&t, &DrlParams::default());
    let d = |a: usize, b: usize| libm::hypotf(x[a] - x[b], y[a] - y[b]);
    assert!(d(0, 1) < d(0, 3), "{} vs {}", d(0, 1), d(0, 3));
}

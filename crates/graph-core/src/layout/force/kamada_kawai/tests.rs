use super::{KamadaKawai, KkParams};
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

fn points(t: &Topology) -> Vec<(f64, f64)> {
    match KamadaKawai::run(t, &KkParams::default())
        .expect("finite")
        .nodes
    {
        NodeGeometry::Point { x, y } => x
            .iter()
            .zip(&y)
            .map(|(&a, &b)| (a.into(), b.into()))
            .collect(),
        other => panic!("point geometry, got {other:?}"),
    }
}

fn dist(p: &[(f64, f64)], i: usize, j: usize) -> f64 {
    let (dx, dy) = (p[i].0 - p[j].0, p[i].1 - p[j].1);
    f64::sqrt(dx * dx + dy * dy)
}

#[test]
fn a_path_of_three_has_pinned_coordinates() {
    let bits: Vec<u32> = points(&graph(3, &[(0, 1), (1, 2)]))
        .iter()
        .flat_map(|p| [(p.0 as f32).to_bits(), (p.1 as f32).to_bits()])
        .collect();
    assert_eq!(
        bits,
        [
            1041122087, 1066450165, 3188014170, 1050508454, 3201064792, 3204709641
        ]
    );
}

#[test]
fn a_path_is_stretched_to_its_hop_length() {
    // d_max = 2, so one hop is sqrt(3) / 2 long and the ends are sqrt(3) apart.
    let p = points(&graph(3, &[(0, 1), (1, 2)]));
    let hop = f64::sqrt(3.0) / 2.0;
    assert!((dist(&p, 0, 1) - hop).abs() < 1e-4, "{}", dist(&p, 0, 1));
    assert!((dist(&p, 0, 2) - 2.0 * hop).abs() < 1e-4);
}

#[test]
fn empty_and_single_node_graphs_are_finite() {
    let empty = KamadaKawai::run(&empty_model(), &KkParams::default()).expect("empty");
    assert_eq!(
        empty.nodes,
        NodeGeometry::Point {
            x: vec![],
            y: vec![]
        }
    );
    assert_eq!(points(&graph(1, &[])), [(f64::from(0.36f32), 0.0)]);
}

#[test]
fn edgeless_and_disconnected_graphs_stay_finite() {
    for t in [graph(4, &[]), graph(5, &[(0, 1), (2, 3)])] {
        assert!(
            points(&t)
                .iter()
                .all(|p| p.0.is_finite() && p.1.is_finite())
        );
    }
}

#[test]
fn no_randomness_the_run_is_repeatable() {
    let t = graph(8, &[(0, 1), (1, 2), (2, 3), (3, 0), (4, 5)]);
    assert_eq!(points(&t), points(&t));
}

#[test]
fn epsilon_stops_the_descent_early() {
    let t = graph(6, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]);
    let stopped = KkParams {
        epsilon: 1e12,
        ..KkParams::default()
    };
    let start = KamadaKawai::run(
        &t,
        &KkParams {
            maxiter: Some(0),
            ..KkParams::default()
        },
    )
    .unwrap();
    assert_eq!(KamadaKawai::run(&t, &stopped).unwrap(), start);
}

use super::energy::{Field, delta};
use super::{DavidsonHarel, DhParams};
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

fn points(t: &Topology, params: DhParams) -> (Vec<f32>, Vec<f32>) {
    match DavidsonHarel::run(t, &params).expect("finite").nodes {
        NodeGeometry::Point { x, y } => (x, y),
        other => panic!("point geometry, got {other:?}"),
    }
}

#[test]
fn a_path_of_three_has_pinned_coordinates() {
    let (x, y) = points(&graph(3, &[(0, 1), (1, 2)]), DhParams::default());
    let bits: Vec<u32> = x.iter().chain(&y).map(|v| v.to_bits()).collect();
    assert_eq!(
        bits,
        [
            3238695016, 3231728135, 3233664090, 3238695016, 3237007480, 3228339621
        ]
    );
}

#[test]
fn empty_single_and_edgeless_graphs_work() {
    assert!(points(&empty_model(), DhParams::default()).0.is_empty());
    assert_eq!(points(&graph(1, &[]), DhParams::default()).0.len(), 1);
    assert_eq!(points(&graph(4, &[]), DhParams::default()).0.len(), 4);
}

#[test]
fn every_point_stays_on_the_canvas() {
    let t = graph(9, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 0), (5, 6), (7, 8)]);
    let (x, y) = points(&t, DhParams::default());
    let limit = 5.0 * 3.0 + 1e-3;
    assert!(x.iter().chain(&y).all(|v| v.abs() <= limit));
}

#[test]
fn the_seed_changes_the_result_and_a_seed_repeats() {
    let t = graph(5, &[(0, 1), (1, 2), (3, 4)]);
    let a = points(&t, DhParams::default());
    let b = points(
        &t,
        DhParams {
            seed: 4,
            ..DhParams::default()
        },
    );
    assert_ne!(a, b);
    assert_eq!(a, points(&t, DhParams::default()));
}

#[test]
fn fine_tuning_rounds_run_and_stay_finite() {
    let p = DhParams {
        fineiter: 2,
        ..DhParams::default()
    };
    let (x, y) = points(&graph(4, &[(0, 1), (1, 2), (2, 3)]), p);
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
}

#[test]
fn zero_rounds_return_the_random_start() {
    let p = DhParams {
        maxiter: 0,
        ..DhParams::default()
    };
    let (x, _) = points(&graph(3, &[(0, 1)]), p);
    assert_eq!(x.len(), 3);
}

#[test]
fn edge_length_energy_rewards_a_shorter_edge() {
    let pos = [[0.0, 0.0], [10.0, 0.0]];
    let adj = vec![vec![1], vec![0]];
    let edges = [(0, 1)];
    let field = Field {
        pos: &pos,
        adj: &adj,
        edges: &edges,
        half_width: 50.0,
    };
    let mut w = DhParams::default().weights;
    (w.node_dist, w.edge_crossings, w.node_edge_dist) = (0.0, 0.0, 0.0);
    let e = delta(&field, &w, 0, ([0.0, 0.0], [4.0, 0.0]));
    assert_eq!(e, 36.0 - 100.0);
}

#[test]
fn a_crossing_costs_one_and_parallel_segments_none() {
    let pos = [[0.0, 0.0], [10.0, 0.0], [5.0, -5.0], [5.0, 5.0]];
    let adj = vec![vec![1], vec![0], vec![3], vec![2]];
    let edges = [(0, 1), (2, 3)];
    let field = Field {
        pos: &pos,
        adj: &adj,
        edges: &edges,
        half_width: 50.0,
    };
    let mut w = DhParams::default().weights;
    (w.node_dist, w.edge_lengths, w.node_edge_dist) = (0.0, 0.0, 0.0);
    // old vs new position: same crossing, gained a crossing, lost a crossing
    assert_eq!(delta(&field, &w, 0, ([0.0, 0.0], [0.0, 0.0])), 0.0);
    assert_eq!(delta(&field, &w, 0, ([0.0, 20.0], [0.0, 0.0])), 1.0);
    assert_eq!(delta(&field, &w, 0, ([0.0, 0.0], [20.0, 0.0])), -1.0);
}

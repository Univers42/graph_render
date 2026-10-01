use super::{Graphopt, GraphoptParams};
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

fn points(t: &Topology, params: GraphoptParams) -> (Vec<f32>, Vec<f32>) {
    match Graphopt::run(t, &params).expect("finite").nodes {
        NodeGeometry::Point { x, y } => (x, y),
        other => panic!("point geometry, got {other:?}"),
    }
}

#[test]
fn a_path_of_three_has_pinned_coordinates() {
    let (x, y) = points(&graph(3, &[(0, 1), (1, 2)]), GraphoptParams::default());
    let bits: Vec<u32> = x.iter().chain(&y).map(|v| v.to_bits()).collect();
    assert_eq!(
        bits,
        [
            3231847283, 1067033201, 1090359973, 3251779549, 1067333698, 1105535153
        ]
    );
}

#[test]
fn empty_and_single_node_graphs_work() {
    let (x, _) = points(&empty_model(), GraphoptParams::default());
    assert!(x.is_empty());
    let (x, y) = points(&graph(1, &[]), GraphoptParams::default());
    assert_eq!((x.len(), y.len()), (1, 1));
}

#[test]
fn zero_iterations_return_the_start() {
    let p = GraphoptParams {
        niter: 0,
        ..GraphoptParams::default()
    };
    let (x, y) = points(&graph(3, &[(0, 1)]), p);
    let again = points(&graph(3, &[]), p);
    assert_eq!((x, y), again);
}

#[test]
fn a_spring_alone_pulls_two_nodes_together() {
    let p = GraphoptParams {
        node_charge: 0.0,
        ..GraphoptParams::default()
    };
    let (x, y) = points(&graph(2, &[(0, 1)]), p);
    let d = libm::sqrt({
        let (dx, dy) = (f64::from(x[0] - x[1]), f64::from(y[0] - y[1]));
        dx * dx + dy * dy
    });
    assert!(d < 1e-3, "{d}");
}

#[test]
fn repulsion_alone_pushes_two_close_nodes_apart() {
    let p = GraphoptParams {
        niter: 1,
        ..GraphoptParams::default()
    };
    let t = graph(2, &[]);
    let start = points(&t, GraphoptParams { niter: 0, ..p });
    let moved = points(&t, p);
    let gap = |(x, y): &(Vec<f32>, Vec<f32>)| (x[0] - x[1]).abs() + (y[0] - y[1]).abs();
    assert!(gap(&moved) > gap(&start));
}

#[test]
fn a_self_loop_and_a_multi_edge_are_finite() {
    let (x, y) = points(
        &graph(2, &[(0, 0), (0, 1), (1, 0)]),
        GraphoptParams::default(),
    );
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
}

#[test]
fn the_seed_changes_the_result_and_a_seed_repeats() {
    let t = graph(4, &[(0, 1), (2, 3)]);
    let a = points(&t, GraphoptParams::default());
    let b = points(
        &t,
        GraphoptParams {
            seed: 9,
            ..GraphoptParams::default()
        },
    );
    assert_ne!(a, b);
    assert_eq!(a, points(&t, GraphoptParams::default()));
}

#[test]
fn a_displacement_never_exceeds_the_cap() {
    let p = GraphoptParams {
        niter: 1,
        ..GraphoptParams::default()
    };
    let t = graph(3, &[(0, 1), (1, 2)]);
    let start = points(&t, GraphoptParams { niter: 0, ..p });
    let end = points(&t, p);
    for (s, e) in start.0.iter().zip(&end.0).chain(start.1.iter().zip(&end.1)) {
        assert!((e - s).abs() <= 5.0 + 1e-5);
    }
}

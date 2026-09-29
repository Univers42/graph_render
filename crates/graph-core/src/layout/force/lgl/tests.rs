use super::{Lgl, LglParams};
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

fn points(t: &Topology, params: LglParams) -> (Vec<f32>, Vec<f32>) {
    match Lgl::run(t, &params).expect("finite").nodes {
        NodeGeometry::Point { x, y } => (x, y),
        other => panic!("point geometry, got {other:?}"),
    }
}

fn rooted() -> LglParams {
    LglParams {
        root: Some(0),
        ..LglParams::default()
    }
}

fn dist(x: &[f32], y: &[f32], a: usize, b: usize) -> f64 {
    f64::from((x[a] - x[b]).hypot(y[a] - y[b]))
}

#[test]
fn a_path_of_four_has_pinned_coordinates() {
    let (x, y) = points(&graph(4, &[(0, 1), (1, 2), (2, 3)]), rooted());
    let bits: Vec<u32> = x.iter().chain(&y).map(|v| v.to_bits()).collect();
    assert_eq!(
        bits,
        [
            3231897929, 3228974806, 3224025049, 3217098931, 1077091083, 1068111095, 3192320780,
            3218957868
        ]
    );
}

#[test]
fn empty_and_single_node_graphs_work() {
    let (x, _) = points(&empty_model(), LglParams::default());
    assert!(x.is_empty());
    let (x, y) = points(&graph(1, &[]), LglParams::default());
    assert_eq!((x, y), (vec![0.0], vec![0.0]));
}

#[test]
fn the_same_seed_repeats() {
    let t = graph(6, &[(0, 1), (0, 2), (1, 3), (2, 4), (4, 5)]);
    assert_eq!(points(&t, rooted()), points(&t, rooted()));
}

#[test]
fn a_disconnected_graph_stays_finite_and_moves_the_component_apart() {
    let t = graph(5, &[(0, 1), (1, 2), (3, 4)]);
    let (x, y) = points(&t, rooted());
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
    let a = LglParams {
        seed: 9,
        ..rooted()
    };
    assert_ne!(points(&t, a), (x, y));
}

#[test]
fn zero_iterations_leave_the_placement_alone() {
    let t = graph(3, &[(0, 1), (0, 2)]);
    let (x, y) = points(
        &t,
        LglParams {
            maxit: 0,
            ..rooted()
        },
    );
    // Area n^2 = 9, so R = sqrt(9 / pi); with two layers the spacing is R and the two
    // layer-1 children sit at angles 0 and pi around the origin.
    let radius = (9.0 / std::f64::consts::PI).sqrt();
    assert!((dist(&x, &y, 0, 1) - radius).abs() < 1e-5);
    assert!((dist(&x, &y, 0, 2) - radius).abs() < 1e-5);
    assert!((dist(&x, &y, 1, 2) - 2.0 * radius).abs() < 1e-5);
}

#[test]
fn edges_pull_closer_than_unrelated_vertices() {
    let t = graph(6, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]);
    let (x, y) = points(&t, rooted());
    assert!(dist(&x, &y, 2, 3) < dist(&x, &y, 0, 5));
}

#[test]
fn an_out_of_range_root_is_drawn_instead() {
    let t = graph(3, &[(0, 1), (1, 2)]);
    let (x, _) = points(
        &t,
        LglParams {
            root: Some(99),
            ..LglParams::default()
        },
    );
    assert!(x.iter().all(|v| v.is_finite()));
}

#[test]
fn non_positive_settings_fall_back_to_the_defaults() {
    let t = graph(4, &[(0, 1), (1, 2), (2, 3)]);
    let odd = LglParams {
        maxdelta: Some(-1.0),
        area: Some(0.0),
        repulserad: Some(-3.0),
        cellsize: Some(0.0),
        ..rooted()
    };
    assert_eq!(points(&t, odd), points(&t, rooted()));
}

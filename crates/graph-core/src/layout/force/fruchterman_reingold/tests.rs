use super::{FrParams, FruchtermanReingold};
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

fn points(t: &Topology, params: FrParams) -> (Vec<f32>, Vec<f32>) {
    match FruchtermanReingold::run(t, &params).expect("finite").nodes {
        NodeGeometry::Point { x, y } => (x, y),
        other => panic!("point geometry, got {other:?}"),
    }
}

#[test]
fn a_path_of_three_has_pinned_coordinates() {
    let (x, y) = points(&graph(3, &[(0, 1), (1, 2)]), FrParams::default());
    let bits: Vec<u32> = x.iter().chain(&y).map(|v| v.to_bits()).collect();
    assert_eq!(
        bits,
        [
            3208336184, 3202176694, 3188186417, 3215849902, 3196222452, 1062839392
        ]
    );
}

#[test]
fn an_isolated_edge_settles_at_unit_length() {
    let (x, y) = points(&graph(2, &[(0, 1)]), FrParams::default());
    let (dx, dy) = (f64::from(x[0] - x[1]), f64::from(y[0] - y[1]));
    let d = libm::sqrt(dx * dx + dy * dy);
    assert!((d - 1.0).abs() < 1e-2, "distance {d}");
}

#[test]
fn zero_iterations_return_the_seeded_start() {
    let t = graph(4, &[(0, 1)]);
    let a = points(
        &t,
        FrParams {
            niter: 0,
            ..FrParams::default()
        },
    );
    let b = points(
        &t,
        FrParams {
            niter: 0,
            seed: 1,
            ..FrParams::default()
        },
    );
    assert_ne!(a, b);
    assert!(a.0.iter().all(|v| v.abs() <= 1.0), "inside the sqrt(n) box");
}

#[test]
fn empty_and_single_node_graphs_are_finite() {
    let empty = FruchtermanReingold::run(&empty_model(), &FrParams::default()).expect("empty");
    assert_eq!(
        empty.nodes,
        NodeGeometry::Point {
            x: vec![],
            y: vec![]
        }
    );
    let (x, y) = points(&graph(1, &[]), FrParams::default());
    assert!(x[0].is_finite() && y[0].is_finite());
}

#[test]
fn disconnected_components_stay_finite_and_bounded() {
    let t = graph(6, &[(0, 1), (1, 2), (3, 4)]);
    let (x, y) = points(&t, FrParams::default());
    assert!(x.iter().chain(&y).all(|v| v.is_finite() && v.abs() < 50.0));
}

#[test]
fn a_run_is_repeatable_and_the_seed_matters() {
    let t = graph(12, &[(0, 1), (1, 2), (2, 3), (4, 5), (6, 7), (7, 8)]);
    let a = points(&t, FrParams::default());
    assert_eq!(a, points(&t, FrParams::default()));
    assert_ne!(
        a,
        points(
            &t,
            FrParams {
                seed: 9,
                ..FrParams::default()
            }
        )
    );
}

#[test]
fn coincident_start_is_separated_without_nan() {
    let t = graph(2, &[]);
    let (x, y) = points(
        &t,
        FrParams {
            niter: 3,
            start_temp: Some(0.0),
            seed: 0,
            dim: 2,
        },
    );
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
}

/// The 3D arm is this kernel at `dim = 3`, and both halves of that have to be true: the
/// geometry carries a z column (so a snapshot of it is `dim = 1`), **and** the third
/// coordinate is a real one rather than a column of zeros — a label on a 2D picture would
/// satisfy the first half alone.
#[test]
fn the_3d_arm_carries_a_live_z_column() {
    let t = graph(6, &[(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)]);
    let flat = FruchtermanReingold::run(&t, &FrParams::default()).expect("2D runs");
    let solid = super::run_3d(&t, &FrParams::default()).expect("3D runs");
    assert!(flat.z.is_none(), "the 2D arm carries no z column");
    let z = solid.z.expect("the 3D arm carries one");
    assert_eq!(z.len(), 6, "one z per node");
    assert!(z.iter().any(|v| *v != 0.0), "z is not a column of zeros");
    assert!(z.iter().all(|v| v.is_finite()));
}

/// A dimension the kernel does not implement is refused, not clamped: a caller asking for
/// four dimensions and silently getting three would not know it had been refused.
#[test]
fn an_unimplemented_dimension_is_refused_rather_than_clamped() {
    let t = graph(3, &[(0, 1)]);
    let err = FruchtermanReingold::run(
        &t,
        &FrParams {
            dim: 4,
            ..FrParams::default()
        },
    );
    assert!(matches!(
        err,
        Err(crate::stage::StageError::Param { name: "dim", .. })
    ));
}

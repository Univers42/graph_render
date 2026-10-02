//! Tests for `layout.force.fruchterman_reingold_3d`. The drawing must use its third axis, the
//! same seed must give the same bytes, the defaults are the spec's, and the degenerate inputs
//! (nothing, one node, an isolated edge, coincident starts) stay finite.

use super::FruchtermanReingold3D;
use crate::index::{Topology, empty_model, index_model};
use crate::layout::Geometry;
use crate::layout::force::fruchterman_reingold::FrParams;
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

fn columns(g: &Geometry) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    match (&g.nodes, &g.z) {
        (NodeGeometry::Point { x, y }, Some(z)) => (widen(x), widen(y), widen(z)),
        _ => panic!("a point geometry with a z column"),
    }
}

fn widen(column: &[f32]) -> Vec<f64> {
    column.iter().map(|&v| f64::from(v)).collect()
}

fn spread(c: &[f64]) -> f64 {
    let lo = c.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = c.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    hi - lo
}

fn path(n: u32) -> Topology {
    graph(n, &(1..n).map(|i| (i - 1, i)).collect::<Vec<_>>())
}

#[test]
fn the_defaults_are_the_specs() {
    let p = FrParams::default();
    assert_eq!(p.niter, 500, "the spec's default niter");
    assert_eq!(p.start_temp, None, "None means sqrt(n) / 10");
}

#[test]
fn the_output_spreads_on_all_three_axes() {
    let g = FruchtermanReingold3D::run(&path(14), &FrParams::default()).expect("finite geometry");
    let (x, y, z) = columns(&g);
    for (name, c) in [("x", &x), ("y", &y), ("z", &z)] {
        assert!(
            spread(c) > 0.0,
            "column {name} is flat: a 3-D layout must use z"
        );
    }
}

#[test]
fn the_same_seed_gives_the_same_bytes() {
    let t = path(9);
    let first = FruchtermanReingold3D::run(&t, &FrParams::default()).expect("first");
    let second = FruchtermanReingold3D::run(&t, &FrParams::default()).expect("second");
    assert_eq!(columns(&first), columns(&second));
}

#[test]
fn the_seed_matters() {
    let t = path(9);
    let a = FruchtermanReingold3D::run(
        &t,
        &FrParams {
            seed: 0,
            ..FrParams::default()
        },
    );
    let b = FruchtermanReingold3D::run(
        &t,
        &FrParams {
            seed: 9,
            ..FrParams::default()
        },
    );
    assert_ne!(columns(&a.expect("a")), columns(&b.expect("b")));
}

#[test]
fn zero_iterations_return_the_seeded_start_inside_the_box() {
    let t = path(7);
    let p = FrParams {
        niter: 0,
        ..FrParams::default()
    };
    let (x, _, _) = columns(&FruchtermanReingold3D::run(&t, &p).expect("start"));
    let side = libm::sqrt(7.0);
    assert!(
        x.iter()
            .all(|v| v.abs() <= side / 2.0 + f64::from(f32::EPSILON))
    );
}

#[test]
fn an_isolated_edge_settles_at_its_own_length() {
    let t = graph(2, &[(0, 1)]);
    let (x, y, z) = columns(&FruchtermanReingold3D::run(&t, &FrParams::default()).expect("edge"));
    let d = libm::sqrt((x[0] - x[1]).powi(2) + (y[0] - y[1]).powi(2) + (z[0] - z[1]).powi(2));
    assert!(
        (d - 1.0).abs() < 1e-2,
        "the 2-D equilibrium length, in 3-D too: {d}"
    );
}

#[test]
fn empty_and_single_node_graphs_are_finite() {
    let empty = FruchtermanReingold3D::run(&empty_model(), &FrParams::default()).expect("empty");
    assert!(matches!(&empty.nodes, NodeGeometry::Point { x, y } if x.is_empty() && y.is_empty()));
    let one = FruchtermanReingold3D::run(&graph(1, &[]), &FrParams::default()).expect("one node");
    let (x, y, z) = columns(&one);
    assert_eq!((x.len(), y.len(), z.len()), (1, 1, 1));
    assert!(x[0].is_finite() && y[0].is_finite() && z[0].is_finite());
}

#[test]
fn a_disconnected_graph_stays_finite_and_uses_its_z() {
    // Two components: the `far` pair term runs, and it is where the spec's decision about
    // igraph's mistyped 3-D axis lives (this port folds z into z).
    let t = graph(6, &[(0, 1), (2, 3)]);
    let g = FruchtermanReingold3D::run(&t, &FrParams::default()).expect("finite geometry");
    let (x, y, z) = columns(&g);
    assert!(x.iter().chain(&y).chain(&z).all(|v| v.is_finite()));
    assert!(
        spread(&z) > 0.0,
        "the disconnected correction must not flatten z"
    );
}

#[test]
fn coincident_starts_are_separated_without_nan() {
    let p = FrParams {
        niter: 3,
        start_temp: Some(0.0),
        ..FrParams::default()
    };
    let t = graph(2, &[]);
    let (x, y, z) = columns(&FruchtermanReingold3D::run(&t, &p).expect("no division by zero"));
    assert!(x.iter().chain(&y).chain(&z).all(|v| v.is_finite()));
}

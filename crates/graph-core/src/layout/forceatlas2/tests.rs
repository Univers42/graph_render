//! Black-box only: `Fa2State`'s fields are private to `state.rs`, so these drive the
//! public [`Stage`] surface, the same as any other caller would.

use super::{Fa2Params, ForceAtlas2};
use crate::index::index_model;
use crate::records::build::{edge, node};
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

fn line(n: u32) -> crate::index::Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (1..n)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

#[test]
fn a_small_graph_settles_to_finite_positions() {
    let geometry = ForceAtlas2::run(&line(20), &Fa2Params::default()).expect("finite");
    let NodeGeometry::Point { x, y } = geometry.nodes else {
        panic!("point geometry")
    };
    assert_eq!((x.len(), y.len()), (20, 20));
    assert!(x.iter().chain(&y).all(|v| v.is_finite()));
}

#[test]
fn an_empty_graph_produces_empty_geometry() {
    let t = crate::index::empty_model();
    let geometry = ForceAtlas2::run(&t, &Fa2Params::default()).expect("empty is finite");
    assert_eq!(
        geometry.nodes,
        NodeGeometry::Point {
            x: vec![],
            y: vec![]
        }
    );
}

#[test]
fn the_same_topology_and_seed_settle_to_the_same_geometry_run_to_run() {
    let t = line(25);
    let a = ForceAtlas2::run(&t, &Fa2Params::default()).expect("finite");
    let b = ForceAtlas2::run(&t, &Fa2Params::default()).expect("finite");
    assert_eq!(
        a, b,
        "pure: the same topology and seed, the same run, every time"
    );
}

#[test]
fn a_single_node_does_not_divide_by_zero() {
    let (nodes, edges) = (vec![node("solo", "")], vec![]);
    let t = index_model(&nodes, &edges).expect("fits");
    let geometry = ForceAtlas2::run(&t, &Fa2Params::default()).expect("finite");
    let NodeGeometry::Point { x, y } = geometry.nodes else {
        panic!("point geometry")
    };
    assert!(x[0].is_finite() && y[0].is_finite());
}

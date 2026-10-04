//! The `layout.force.yifan_hu.3d` arm's own suite, beside `tests.rs`.
//!
//! The rows a 3D multilevel force arm can fail while every structural test still passes are
//! the ones here: an arm that returns the 2D picture with a z column attached, an arm that
//! returns a z column of zeros, and an arm whose bytes move between two runs of the same
//! topology.

use super::super::barnes_hut::BarnesHut;
use super::{ID_2Z, ID_3D, YifanHu, hierarchy, run_3d};
use crate::index::Topology;
use crate::layout::force::ForceParams;
use crate::layout::force::simple_graph;
use crate::records::build::{edge, node};
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

/// A path of `n` nodes, the 2D suite's own fixture.
fn path(n: u32) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (1..n)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    crate::index::index_model(&nodes, &edges).expect("fits")
}

fn columns(t: &Topology) -> (Vec<f32>, Vec<f32>, Option<Vec<f32>>) {
    let g = run_3d(t, &ForceParams::default()).expect("finite");
    let NodeGeometry::Point { x, y } = g.nodes else {
        panic!("force layout is point geometry")
    };
    (x, y, g.z)
}

/// A column's largest minus its smallest.
fn spread(v: &[f32]) -> f32 {
    let lo = v.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = v.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    hi - lo
}

#[test]
fn the_3d_arm_has_its_own_id_and_the_2d_arms_keep_theirs() {
    assert_eq!(ID_3D, "layout.force.yifan_hu.3d");
    assert_eq!(ID_2Z, "layout.force.yifan_hu.2z");
    assert_eq!(<YifanHu as Stage>::ID, "layout.force.yifan_hu");
    let ids = [<YifanHu as Stage>::ID, ID_2Z, ID_3D, BarnesHut::ID];
    let mut sorted = ids;
    sorted.sort_unstable();
    assert_eq!(sorted.len(), 4, "four distinct ids: {ids:?}");
    for pair in sorted.windows(2) {
        assert_ne!(pair[0], pair[1], "duplicate id {pair:?}");
    }
}

#[test]
fn the_3d_arm_returns_three_finite_columns_of_equal_length() {
    let t = path(40);
    let (x, y, z) = columns(&t);
    let z = z.expect("a 3D arm carries a z column");
    assert_eq!((x.len(), y.len(), z.len()), (40, 40, 40));
    assert!(x.iter().chain(&y).chain(&z).all(|v| v.is_finite()));
}

#[test]
fn the_3d_arm_is_repeatable() {
    let t = path(40);
    assert_eq!(columns(&t), columns(&t), "same topology, same bytes");
}

/// The load-bearing row: the 3D arm is a **third** dimension of the force run, not the 2D
/// picture with a column attached. Both the in-plane columns and the z column must differ
/// from the 2D arm's own.
#[test]
fn the_3d_arm_is_not_the_2d_run_with_a_column_pasted_onto_it() {
    let t = path(60);
    let (x3, y3, z3) = columns(&t);
    let z3 = z3.expect("a z column");
    let planar = BarnesHut::run(&t, &ForceParams::default()).expect("finite");
    let NodeGeometry::Point { x, y } = planar.nodes else {
        panic!("force layout is point geometry")
    };
    assert!(x3.iter().any(|v| v != &0.0), "x is live");
    assert_ne!(x3, x, "the in-plane columns move when z steers the run");
    assert_ne!(y3, y, "on both in-plane axes");
    let z_span = spread(&z3);
    assert!(z_span > 1.0, "z spread {z_span} is a layout, not noise");
    assert!(z3.iter().any(|&v| v != 0.0), "z is not a column of zeros");
}

/// The 3D arm shares the 2D arm's coarsening: the same hierarchy, because the hierarchy is a
/// property of the graph and not of the dimension.
#[test]
fn the_3d_arm_coarsens_on_the_2d_arm_s_levels() {
    let t = path(200);
    let (levels, maps) = hierarchy(simple_graph(&t), 200);
    let sizes: Vec<u32> = levels.iter().map(|l| l.1).collect();
    assert_eq!(sizes, [200, 100, 50, 25, 13]);
    assert_eq!(maps.len(), 4);
}

#[test]
fn an_empty_graph_lays_out_to_three_empty_columns() {
    let t = crate::index::empty_model();
    let (x, y, z) = columns(&t);
    assert_eq!(x, Vec::<f32>::new());
    assert_eq!(y, Vec::<f32>::new());
    assert_eq!(z, Some(Vec::new()));
}

#[test]
fn a_graph_of_one_node_lays_out_to_one_point_on_each_axis() {
    let t = path(1);
    let (x, y, z) = columns(&t);
    let z = z.expect("a z column");
    assert_eq!((x.len(), y.len(), z.len()), (1, 1, 1));
    assert!(x[0].is_finite() && y[0].is_finite() && z[0].is_finite());
}

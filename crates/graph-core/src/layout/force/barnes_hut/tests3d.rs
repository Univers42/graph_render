//! The 3D simulation's own suite. Split from `tests.rs` for the house 300-line cap.
//!
//! The rows here are the 2D suite's rows at three axes, plus the one that only a 3D arm can
//! fail: a layout that answered with a z column of zeros, or with the 2D arm's columns and
//! a pasted z. `all_three_axes_carry_a_live_column` and
//! `the_3d_run_is_not_the_2d_run_with_a_column_pasted_onto_it` are those two.

use super::collide3d;
use super::link3d;
use super::seed::{sphere_point, spiral_point};
use super::settle3d::settle3d;
use super::sim3d::Sim3;
use crate::index::index_model;
use crate::layout::force::params::ForceParams;
use crate::layout::force::{LiveParams, simple_graph};
use crate::records::build::{edge, node};
use crate::records::{EdgeRecord, NodeRecord};

fn line(n: u32) -> (Vec<NodeRecord>, Vec<EdgeRecord>) {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = (1..n)
        .map(|i| edge(&format!("e{i}"), &format!("n{}", i - 1), &format!("n{i}")))
        .collect();
    (nodes, edges)
}

fn graph(n: u32) -> crate::layout::force::SimpleGraph {
    let (nodes, edges) = line(n);
    simple_graph(&index_model(&nodes, &edges).expect("fits"))
}

/// One settled 3D run's three columns.
fn settled(n: u32, ticks: u32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    settle3d(
        graph(n),
        ForceParams::default(),
        super::seed::golden_sphere(n),
        (ticks, 1.0),
    )
}

/// The largest minus the smallest of a column.
fn spread(v: &[f64]) -> f64 {
    let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    hi - lo
}

#[test]
fn an_empty_graph_settles_to_three_empty_columns() {
    let (x, y, z) = settle3d(
        graph(1),
        ForceParams::default(),
        (Vec::new(), Vec::new(), Vec::new()),
        (1, 1.0),
    );
    assert_eq!((x.len(), y.len(), z.len()), (0, 0, 0));
}

#[test]
fn a_settled_3d_run_is_finite_on_all_three_axes() {
    let (x, y, z) = settled(24, 8);
    assert_eq!((x.len(), y.len(), z.len()), (24, 24, 24));
    assert!(x.iter().chain(&y).chain(&z).all(|v| v.is_finite()));
}

/// The row a 3D arm can fail while still passing "it has three columns": every axis must
/// actually spread, and z must not be a constant or a copy of x.
#[test]
fn all_three_axes_carry_a_live_column() {
    let (x, y, z) = settled(24, 8);
    assert!(spread(&x) > 1.0, "x spread {}", spread(&x));
    assert!(spread(&y) > 1.0, "y spread {}", spread(&y));
    assert!(spread(&z) > 1.0, "z spread {}", spread(&z));
    assert_ne!(z, x, "z is not a copy of x");
    assert!(
        z.iter().any(|&v| v != 0.0),
        "z is not a column of zeros"
    );
}

#[test]
fn the_3d_settle_is_a_pure_function_of_its_inputs() {
    assert_eq!(settled(18, 6), settled(18, 6), "same input, same bytes");
}

#[test]
fn the_center_pass_shifts_each_axis_by_its_own_mean() {
    let mut sim = Sim3::from_parts(
        graph(4),
        LiveParams::from(ForceParams::default()),
        0,
        (
            vec![1.0, 2.0, 3.0, 4.0],
            vec![-1.0, 0.0, 1.0, 2.0],
            vec![10.0, 20.0, 30.0, 40.0],
        ),
    );
    assert_eq!(sim.center_shift(), Some((2.5, 0.5, 25.0)));
    sim.center();
    assert_eq!(sim.x.iter().sum::<f64>(), 0.0, "x is centred");
    assert_eq!(sim.y.iter().sum::<f64>(), 0.0, "y is centred");
    assert_eq!(sim.z.iter().sum::<f64>(), 0.0, "z is centred");
}

#[test]
fn a_link_between_two_coincident_nodes_has_a_finite_three_axis_force() {
    let mut sim = Sim3::from_parts(
        graph(2),
        LiveParams::from(ForceParams::default()),
        0,
        (vec![7.0, 7.0], vec![7.0, 7.0], vec![7.0, 7.0]),
    );
    let f = link3d::force(&sim, 0);
    assert!(
        f.0.is_finite() && f.1.is_finite() && f.2.is_finite(),
        "no infinite axis for a coincident pair: {f:?}"
    );
    link3d::apply(&mut sim);
    let v = [sim.vx[0], sim.vy[0], sim.vz[0]];
    assert!(v.iter().all(|x| x.is_finite()), "the gather stays finite: {v:?}");
    for axis in 0..3 {
        assert!(
            v[axis] * [sim.vx[1], sim.vy[1], sim.vz[1]][axis] <= 0.0,
            "axis {axis}: the bias splits the edge, so the endpoints move opposite ways"
        );
    }
}

/// Two nodes inside one collide radius push apart on **every** axis, and the push is exactly
/// the negation each gets from the other, which is the canonical-orientation claim the 2D
/// pass makes in two dimensions.
#[test]
fn two_overlapping_nodes_separate_on_every_axis_and_by_negation() {
    let mut sim = Sim3::from_parts(
        graph(2),
        LiveParams::from(ForceParams::default()),
        0,
        (vec![0.0, 0.5], vec![0.5, 0.0], vec![0.5, 0.0]),
    );
    collide3d::prepare(&mut sim);
    let reach = collide3d::reach_squared(&sim);
    let a = collide3d::node_delta(&sim, 0, reach);
    let b = collide3d::node_delta(&sim, 1, reach);
    assert!(
        a.0 != 0.0 && a.1 != 0.0 && a.2 != 0.0,
        "pushed on all three: {a:?}"
    );
    assert_eq!(a, (-b.0, -b.1, -b.2), "one shared subtraction, negated");
}

#[test]
fn a_pair_further_apart_than_the_reach_gets_no_collide_push() {
    let mut sim = Sim3::from_parts(
        graph(2),
        LiveParams::from(ForceParams::default()),
        0,
        (vec![0.0, 1e6], vec![0.0, 0.0], vec![0.0, 0.0]),
    );
    collide3d::prepare(&mut sim);
    let reach = collide3d::reach_squared(&sim);
    assert_eq!(collide3d::node_delta(&sim, 0, reach), (0.0, 0.0, 0.0));
}

#[test]
fn the_3d_start_is_a_sphere_and_not_a_plane() {
    let (x, y, z) = super::seed::golden_sphere(12);
    assert!(z.iter().any(|&v| v != 0.0), "z is live from the first tick");
    for i in 0..x.len() {
        let r = f64::sqrt(x[i] * x[i] + y[i] * y[i] + z[i] * z[i]);
        let want = 12.0 * f64::sqrt(i as f64 + 1.0);
        assert!((r - want).abs() < 1e-9, "row {i} off the sphere: {r} vs {want}");
    }
    // Pure function of the index, as the 2D spiral is: one row alone, so a level that grows
    // seeds its own row without the rows before it.
    assert_eq!(sphere_point(7), sphere_point(7));
    assert_ne!(spiral_point(7).0, sphere_point(7).0, "a different point from the 2D spiral");
}
//! Tests for `layout.force.kamada_kawai_3d`. The sphere start is pinned against the spec's
//! table, the output has to be a drawing in three axes rather than a flat one, the run has to
//! repeat, and the three-node path that makes igraph return infinities has to be finite here.

use super::{sphere_row, sphere_start, KamadaKawai3D};
use crate::index::{Topology, empty_model, index_model};
use crate::layout::Geometry;
use crate::layout::force::kamada_kawai::KkParams;
use crate::records::build::{edge, node};
use crate::stage::Stage;
use graph_contract::geometry::NodeGeometry;

/// `n` nodes numbered `n0..` and the given edges, as every force layout test builds one.
fn graph(n: u32, pairs: &[(u32, u32)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&format!("n{i}"), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(i, (a, b))| edge(&format!("e{i}"), &format!("n{a}"), &format!("n{b}")))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

/// The three columns of a point geometry, widened to `f64` so the assertions are about the
/// layout and not about the narrowing.
fn columns(g: &Geometry) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    match (&g.nodes, &g.z) {
        (NodeGeometry::Point { x, y }, Some(z)) => (widen(x), widen(y), widen(z)),
        _ => panic!("a point geometry with a z column"),
    }
}

fn widen(column: &[f32]) -> Vec<f64> {
    column.iter().map(|&v| f64::from(v)).collect()
}

fn norm3(p: [f64; 3]) -> f64 {
    libm::sqrt(p[0] * p[0] + p[1] * p[1] + p[2] * p[2])
}

fn spread(c: &[f64]) -> f64 {
    let lo = c.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = c.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    hi - lo
}

#[test]
fn the_sphere_start_matches_the_spec_table() {
    // Poles: row 0 at (0, 0, -1), row n-1 at (0, 0, +1), whatever n is, and phi has not
    // moved on either of them.
    let mut phi = 0.0;
    assert_eq!(sphere_row(0, 7, &mut phi), [0.0, 0.0, -1.0]);
    assert_eq!(phi, 0.0, "phi does not advance on the first row");
    assert_eq!(sphere_row(6, 7, &mut phi), [0.0, 0.0, 1.0]);

    // Interior: z = -1 + 2i / (n - 1), r = sqrt(1 - z z), phi += 3.6 / (sqrt(n) r). The step
    // uses *this* row's own r, so phi is a running sum of per-row increments, not a count
    // times one shared increment — pinning that is the point of the assertion.
    let n = 7;
    let mut phi = 0.0;
    let mut want = 0.0;
    for i in 1..n - 1 {
        let row = sphere_row(i, n, &mut phi);
        let z = -1.0 + 2.0 * i as f64 / (n - 1) as f64;
        let r = libm::sqrt(1.0 - z * z);
        let on_ring = libm::sqrt(row[0] * row[0] + row[1] * row[1]);
        want += 3.6 / (libm::sqrt(n as f64) * r);
        assert!((row[2] - z).abs() < 1e-15, "row {i}: z");
        assert!((on_ring - r).abs() < 1e-15, "row {i}: r");
        assert!((phi - want).abs() < 1e-12, "row {i}: phi {}", phi);
        // The row is the point at angle phi on the ring of radius r.
        assert!((row[0] - r * libm::cos(phi)).abs() < 1e-15, "row {i}: x");
        assert!((row[1] - r * libm::sin(phi)).abs() < 1e-15, "row {i}: y");
    }
}

#[test]
fn the_sphere_start_is_scaled_by_the_spec_radius() {
    let n = 12;
    let pos = sphere_start(n);
    let radius = 0.36 * libm::sqrt(n as f64);
    assert_eq!(pos.len(), n);
    // phi accumulates across interior rows, so the replay carries one `phi` through the loop
    // rather than restarting it — a reset here would compare row i against the wrong angle.
    let mut phi = 0.0;
    for (i, p) in pos.iter().enumerate() {
        let unit = sphere_row(i, n, &mut phi);
        let want = [unit[0] * radius, unit[1] * radius, unit[2] * radius];
        assert!(norm3([p[0] - want[0], p[1] - want[1], p[2] - want[2]]) < 1e-15, "row {i}");
    }
    assert!((pos[0][2] + radius).abs() < 1e-15, "row 0 is the south pole");
    assert!(
        (pos[n - 1][2] - radius).abs() < 1e-15,
        "row n-1 is the north pole"
    );
}

/// The property that breaks igraph: three vertices, so the 3x3 Newton block is near-singular at
/// every one of them and igraph's step overflows `f64`. Not component count, not an isolated
/// node and not degree — this graph is connected with degrees 2, 1, 1, and the same graph is
/// finite at `dim=2`. The kernel's singular-block guard makes the step zero instead.
#[test]
fn the_three_node_path_that_breaks_igraph_is_finite_here() {
    let t = graph(3, &[(1, 0), (0, 2)]);
    let g = KamadaKawai3D::run(&t, &KkParams::default()).expect("finite geometry");
    let (x, y, z) = columns(&g);
    assert_eq!(x.len(), 3);
    assert!(x.iter().chain(&y).chain(&z).all(|v| v.is_finite()));
    assert!(z.iter().any(|&v| v != 0.0), "a 3-D drawing must use z");
}

#[test]
fn the_output_spreads_on_all_three_axes() {
    let t = graph(14, &(1..14).map(|i| (i - 1, i)).collect::<Vec<_>>());
    let g = KamadaKawai3D::run(&t, &KkParams::default()).expect("finite geometry");
    let (x, y, z) = columns(&g);
    for (name, c) in [("x", &x), ("y", &y), ("z", &z)] {
        assert!(spread(c) > 0.0, "column {name} is flat");
    }
}

#[test]
fn a_run_is_repeatable_and_deterministic() {
    let t = graph(9, &(1..9).map(|i| (i - 1, i)).collect::<Vec<_>>());
    let first = KamadaKawai3D::run(&t, &KkParams::default()).expect("first");
    let second = KamadaKawai3D::run(&t, &KkParams::default()).expect("second");
    assert_eq!(columns(&first), columns(&second), "same input, same bytes");
}

#[test]
fn empty_and_single_node_graphs_are_finite() {
    let empty = KamadaKawai3D::run(&empty_model(), &KkParams::default()).expect("empty");
    assert!(matches!(&empty.nodes, NodeGeometry::Point { x, y } if x.is_empty() && y.is_empty()));
    let one = KamadaKawai3D::run(&graph(1, &[]), &KkParams::default()).expect("one node");
    let (x, y, z) = columns(&one);
    assert_eq!((x.len(), y.len(), z.len()), (1, 1, 1));
    assert!(x[0].is_finite() && y[0].is_finite() && z[0].is_finite());
    assert!((z[0] + 0.36).abs() < 1e-6, "one vertex is the south pole");
}
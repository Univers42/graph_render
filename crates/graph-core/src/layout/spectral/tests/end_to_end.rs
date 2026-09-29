//! End-to-end `layout::spectral::run(&Topology)` tests: the C6 adjacency rule, the
//! required `C_300` circle layout, multi-component packing and C12's diagnostics,
//! determinism across both tiers, and two structural invariants — `star`'s center at
//! the origin, `path`'s monotonic first axis — that hold for *any* orthonormal basis a
//! solver's degenerate eigenspace happens to return. Per-component eigenvalue accuracy
//! is `../tests.rs`'s concern; this file is the pipeline around it.

use super::super::*;
use crate::index::index_model;
use crate::records::build::{edge, node};
use graph_contract::geometry::NodeGeometry;

fn topology(n: usize, pairs: &[(usize, usize)]) -> Topology {
    let nodes: Vec<_> = (0..n).map(|i| node(&i.to_string(), "")).collect();
    let edges: Vec<_> = pairs
        .iter()
        .enumerate()
        .map(|(k, &(a, b))| edge(&format!("e{k}"), &a.to_string(), &b.to_string()))
        .collect();
    index_model(&nodes, &edges).expect("fits")
}

fn cycle_pairs(n: usize) -> Vec<(usize, usize)> {
    (0..n).map(|i| (i, (i + 1) % n)).collect()
}

fn path_pairs(n: usize) -> Vec<(usize, usize)> {
    (0..n - 1).map(|i| (i, i + 1)).collect()
}

fn star_pairs(n: usize) -> Vec<(usize, usize)> {
    (1..n).map(|i| (0, i)).collect()
}

fn points(geometry: &Geometry) -> (&[f32], &[f32]) {
    match &geometry.nodes {
        NodeGeometry::Point { x, y } => (x, y),
        other => panic!("expected Point geometry, got {other:?}"),
    }
}

#[test]
fn simple_neighbors_collapses_self_loops_and_parallel_edges_c6() {
    // 0-1 twice (both directions), a 1-1 self-loop, and 1-2 once: the collapsed,
    // undirected result is exactly {0: [1], 1: [0, 2], 2: [1]}.
    let t = topology(3, &[(0, 1), (1, 0), (1, 1), (1, 2)]);
    assert_eq!(simple_neighbors(&t), vec![vec![1], vec![0, 2], vec![1]]);
}

#[test]
fn c_300_lays_out_as_a_circle_equal_radii_uniform_angles() {
    let t = topology(300, &cycle_pairs(300));
    let (geometry, reports) = run(&t).expect("the one component solves");
    assert_eq!(reports.len(), 1);
    assert_eq!(
        reports[0].tier,
        Tier::Lobpcg,
        "n=300 > DENSE_EIG_LIMIT forces LOBPCG"
    );
    assert!(reports[0].solved);

    let (x, y) = points(&geometry);
    let radii: Vec<f64> = x
        .iter()
        .zip(y)
        .map(|(&x, &y)| libm::hypot(f64::from(x), f64::from(y)))
        .collect();
    let mean = radii.iter().sum::<f64>() / radii.len() as f64;
    for (i, &r) in radii.iter().enumerate() {
        assert!(
            (r - mean).abs() / mean < 0.05,
            "node {i}: radius {r} vs mean {mean}"
        );
    }

    let mut angles: Vec<f64> = x
        .iter()
        .zip(y)
        .map(|(&x, &y)| libm::atan2(f64::from(y), f64::from(x)))
        .collect();
    angles.sort_by(f64::total_cmp);
    let want_gap = 2.0 * core::f64::consts::PI / 300.0;
    for w in angles.windows(2) {
        assert!(
            (w[1] - w[0] - want_gap).abs() < 0.02,
            "gap {} vs {want_gap}",
            w[1] - w[0]
        );
    }
    let wrap = angles[0] + 2.0 * core::f64::consts::PI - angles[299];
    assert!(
        (wrap - want_gap).abs() < 0.02,
        "wraparound gap {wrap} vs {want_gap}"
    );
}

#[test]
fn eigen_determinism_run_twice_across_both_tiers() {
    let dense = topology(40, &path_pairs(40));
    assert_eq!(
        run(&dense),
        run(&dense),
        "dense tier: same input bits, same output bits"
    );

    let big = topology(300, &cycle_pairs(300));
    assert_eq!(
        run(&big),
        run(&big),
        "lobpcg tier: same input bits, same output bits"
    );
}

#[test]
fn eigen_determinism_on_degenerate_grid_pins_basis_and_signs() {
    // 6x6 grid: lambda2 == lambda3, the case where an unpinned start vector rotates the basis.
    let pairs: Vec<(usize, usize)> = (0..6)
        .flat_map(|r| (0..6).map(move |c| (r, c)))
        .flat_map(|(r, c)| {
            let i = r * 6 + c;
            let right = (c + 1 < 6).then_some((i, i + 1));
            let down = (r + 1 < 6).then_some((i, i + 6));
            right.into_iter().chain(down)
        })
        .collect();
    let grid = topology(36, &pairs);
    let first = run(&grid);
    assert!(first.is_ok(), "grid must solve");
    assert_eq!(
        first,
        run(&grid),
        "degenerate eigenspace: same bits every run"
    );
}

#[test]
fn disconnected_graph_reports_only_attempted_components_and_places_every_node() {
    // A path of 5, one isolated node, and a triangle: 3 components, one a singleton.
    let mut nodes: Vec<_> = (0..5).map(|i| node(&format!("p{i}"), "")).collect();
    nodes.push(node("iso", ""));
    nodes.extend((0..3).map(|i| node(&format!("t{i}"), "")));
    let mut edges: Vec<_> = (0..4)
        .map(|i| edge(&format!("pe{i}"), &format!("p{i}"), &format!("p{}", i + 1)))
        .collect();
    edges.extend([
        edge("te0", "t0", "t1"),
        edge("te1", "t1", "t2"),
        edge("te2", "t2", "t0"),
    ]);
    let t = index_model(&nodes, &edges).expect("fits");

    let (geometry, reports) = run(&t).expect("both size >= 2 components solve");
    assert_eq!(
        reports.len(),
        2,
        "the isolated node gets no report: nothing was attempted for it"
    );
    assert!(reports.iter().all(|r| r.solved));
    let sizes: Vec<u32> = reports.iter().map(|r| r.size).collect();
    assert_eq!(
        sizes,
        vec![5, 3],
        "reported in discovery order: the path's component first"
    );

    let (x, y) = points(&geometry);
    assert_eq!(x.len(), 9);
    assert!(x.iter().chain(y).all(|v| v.is_finite()));
    for i in 0..9 {
        for j in (i + 1)..9 {
            assert!((x[i], y[i]) != (x[j], y[j]), "{i} and {j} coincide");
        }
    }
}

#[test]
fn star_center_sits_at_the_origin_regardless_of_the_degenerate_eigenspace_basis() {
    // Every eigenvector of a star's smallest nontrivial (degenerate) eigenvalue is zero
    // at the center by construction (the leaf-row eigen-equations force it), so the
    // center lands at the origin under any orthonormal basis a solver returns for it.
    let t = topology(10, &star_pairs(10));
    let (geometry, reports) = run(&t).expect("solves");
    assert!(reports[0].solved);
    let (x, y) = points(&geometry);
    assert!(
        x[0].abs() < 1e-9 && y[0].abs() < 1e-9,
        "center at ({}, {})",
        x[0],
        y[0]
    );
    let leaf_radii: Vec<f64> = (1..10)
        .map(|i| libm::hypot(f64::from(x[i]), f64::from(y[i])))
        .collect();
    assert!(leaf_radii.iter().any(|&r| r > 0.1), "{leaf_radii:?}");
}

#[test]
fn path_lays_out_monotonically_along_the_first_axis() {
    // The path's Fiedler vector, cos(pi*i/n), is strictly monotonic over i = 0..n (less
    // than half a period): true under either sign the solver pins it to.
    let n = 50;
    let t = topology(n, &path_pairs(n));
    let (geometry, _) = run(&t).expect("solves");
    let (x, _) = points(&geometry);
    let increasing = x.windows(2).all(|w| w[0] < w[1]);
    let decreasing = x.windows(2).all(|w| w[0] > w[1]);
    assert!(increasing || decreasing, "{x:?}");
}

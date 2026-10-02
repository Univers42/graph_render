//! `layout.force.spring3d`: the *dimension* is asserted here, not the algorithm.
//!
//! Nothing in this file re-tests Fruchterman–Reingold: `super::tests` does that over the
//! identical code path, and the 2D arm's hashed coordinates are pinned there. What these
//! tests exist for is the one thing that genuinely differs between the two stages — the
//! geometry. A `SPRING_3D` built with `Geometry::planar`, or one whose z column is dropped
//! by a post pass, is a **silent** 2D snapshot: it hashes, it decodes, and it lies. So each
//! assertion below is about the column's presence, its length, its label, and D9's naming.

use super::{Spring3D, SpringParams};
use crate::layout::Geometry;
use crate::layout::coords::probe::graph;
use crate::stage::Stage;
use graph_contract::geometry::{EdgeGeometry, NodeGeometry};
use graph_contract::snapshot::Dim;
use std::collections::HashSet;

/// The stage at its default parameters.
pub(super) fn run(t: &crate::index::Topology) -> Geometry {
    Spring3D::run(t, &SpringParams::default()).expect("runs")
}

/// The three columns, in axis order. Panics rather than returning `None`: a 3D geometry
/// without a z column is the failure this file is about, so it must not be readable past.
pub(super) fn columns(g: &Geometry) -> (&[f32], &[f32], &[f32]) {
    let NodeGeometry::Point { x, y } = &g.nodes else {
        panic!("point nodes");
    };
    (
        x.as_slice(),
        y.as_slice(),
        g.z.as_deref().expect("a z column"),
    )
}

/// `path_edges(n)`: the `n`-node path `0-1-...-n-1`, the tree the port's own module doc
/// names as an analytically-determined case.
pub(super) fn path_edges(n: u32) -> Vec<(u32, u32)> {
    (0..n.saturating_sub(1)).map(|i| (i, i + 1)).collect()
}

/// The stage's own id, distinct from the 2D one. A 3D snapshot that hashed under
/// `layout.force.spring` would silently replace a pinned 2D digest.
#[test]
fn the_3d_id_names_its_own_layout() {
    assert_eq!(Spring3D::ID, "layout.force.spring3d");
    assert_ne!(Spring3D::ID, super::super::ID);
}

/// The whole point of `Geometry::in_space`: a z column of exactly `n` values, and a `dim`
/// that says so. Checked from one node to seventeen, and at zero.
#[test]
fn the_3d_stage_carries_a_z_column_of_the_right_length_and_labelled_3d() {
    for n in [0u32, 1, 2, 3, 8, 17] {
        let t = graph(n, &path_edges(n));
        let g = run(&t);
        let (x, y, z) = columns(&g);
        let want = n as usize;
        assert_eq!(x.len(), want, "n={n}");
        assert_eq!(y.len(), want, "n={n}");
        assert_eq!(
            z.len(),
            want,
            "n={n}: the z column is the dimension, not a decoration"
        );
        assert_eq!(g.dim(), Dim::D3, "n={n} was labelled 2D");
    }
}

/// The geometry is `Point` + `Line` + no notes, as 2D: the dimension moved, the drawing did
/// not. And it narrows to `f32` and is finite throughout, which is what the 2D tests
/// already say of the 2D arm.
#[test]
fn the_3d_stage_keeps_the_point_line_shape_and_narrows_to_f32() {
    let g = run(&graph(6, &path_edges(6)));
    assert_eq!(g.edges, EdgeGeometry::Line);
    assert!(g.notes.is_empty());
    let (x, y, z) = columns(&g);
    assert!(x.iter().chain(y).chain(z).all(|v| v.is_finite()));
}

/// `layout.py:620-624`: `n < 2` returns the centre before any force, and the centre in three
/// dimensions is the origin in **all three** columns. A 2D-shaped early return would leave
/// z absent or absent-in-value while the stage still claimed `Dim::D3`.
#[test]
fn the_3d_centre_is_zero_in_all_three_columns() {
    let one = graph(1, &[]);
    let centre = run(&one);
    let (x, y, z) = columns(&centre);
    assert_eq!(
        (x.to_vec(), y.to_vec(), z.to_vec()),
        (vec![0.0], vec![0.0], vec![0.0])
    );
    // With no iterations too: the early return is before the loop, not after it.
    let none = Spring3D::run(
        &graph(1, &[]),
        &SpringParams {
            iterations: 0,
            ..SpringParams::default()
        },
    )
    .expect("runs");
    assert_eq!(none.dim(), Dim::D3, "one node is still a 3D layout");
    assert_eq!(none.z.as_deref(), Some([0.0].as_slice()));
}

/// An empty graph still emits a present-but-empty z column: the column's *presence* is the
/// dimension, so a degenerate run must not drop it and relabel itself 2D.
#[test]
fn an_empty_graph_is_still_a_3d_geometry() {
    let g = run(&graph(0, &[]));
    assert_eq!(g.z.as_deref(), Some([].as_slice()));
    assert_eq!(g.dim(), Dim::D3);
}

/// The rescale contract of `layout.py:646` at `dim = 3`: the centroid is the origin on
/// every axis, and the largest magnitude over **all three** axes — not just x and y — is
/// exactly `scale`. A kernel that rescaled per-axis, or took the max over two of three
/// columns, fails the second half here and passes the 2D tests.
#[test]
fn the_3d_rescale_spans_the_scale_over_all_three_columns() {
    for n in [2u32, 3, 8, 17] {
        let t = graph(n, &path_edges(n));
        let laid_out = run(&t);
        let (x, y, z) = columns(&laid_out);
        let count = n as f64;
        let mean = |c: &[f32]| c.iter().map(|&v| f64::from(v)).sum::<f64>() / count;
        assert!(
            mean(x).abs() < 1e-3 && mean(y).abs() < 1e-3 && mean(z).abs() < 1e-3,
            "n={n}"
        );
        let extent = x
            .iter()
            .chain(y)
            .chain(z)
            .map(|v| f64::from(*v).abs())
            .fold(0.0, f64::max);
        assert!(
            (extent - 5.0).abs() < 1e-3,
            "n={n} spans {extent} over three columns, not the default scale 5.0"
        );
    }
}

/// The three columns are genuinely three: no two of a node's coordinates agree, and no two
/// nodes share one. A `z` copied out of `x` would fail both halves.
#[test]
fn the_third_column_is_a_coordinate_and_not_a_copy() {
    let t = graph(9, &path_edges(9));
    let laid_out = run(&t);
    let (x, y, z) = columns(&laid_out);
    let agrees = x
        .iter()
        .zip(y)
        .zip(z)
        .any(|((&a, &b), &c)| a == b || b == c || a == c);
    assert!(
        !agrees,
        "a 3D column is a copy of another: {x:?} {y:?} {z:?}"
    );
    let distinct = x
        .iter()
        .chain(y)
        .chain(z)
        .map(|v| v.to_bits())
        .collect::<HashSet<u32>>()
        .len();
    assert_eq!(distinct, 27, "two nodes coincided in 3D");
}

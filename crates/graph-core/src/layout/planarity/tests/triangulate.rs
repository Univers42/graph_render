//! [`triangulate_embedding`]: fan-triangulates every face but the largest, leaving that
//! one as the outer boundary, joining disconnected components first.

use super::graphs::{grid, maximal_planar, outerplanar_fan};
use crate::layout::planarity::{euler_certificate, faces, planar_embedding, triangulate_embedding};

#[test]
fn triangulating_a_square_adds_one_diagonal_leaving_the_larger_face_as_boundary() {
    let embedding =
        planar_embedding(4, &[(0, 1), (1, 2), (2, 3), (0, 3)]).expect("a 4-cycle is planar");
    let (tri, outer) = triangulate_embedding(&embedding);
    assert_eq!(tri.node_count(), 4);
    assert_eq!(
        tri.edge_count(),
        5,
        "one diagonal splits the inner square into two triangles"
    );
    assert!(euler_certificate(&tri));
    assert_eq!(outer.len(), 4, "the untouched face is left as the boundary");
    let all = faces(&tri);
    let mut lens: Vec<u32> = (0..all.len()).map(|i| all.face(i).len() as u32).collect();
    lens.sort_unstable();
    assert_eq!(lens, [3, 3, 4]);
}

#[test]
fn triangulating_an_already_triangular_graph_adds_no_edges() {
    let embedding = planar_embedding(3, &[(0, 1), (1, 2), (0, 2)]).expect("a triangle is planar");
    let (tri, outer) = triangulate_embedding(&embedding);
    assert_eq!(tri.edge_count(), 3);
    assert_eq!(outer.len(), 3);
    assert!(euler_certificate(&tri));
}

#[test]
fn triangulating_two_disjoint_triangles_joins_them_into_one_component() {
    let edges = [(0, 1), (1, 2), (0, 2), (3, 4), (4, 5), (3, 5)];
    let embedding = planar_embedding(6, &edges).expect("two triangles are planar");
    let (tri, _outer) = triangulate_embedding(&embedding);
    assert_eq!(tri.node_count(), 6);
    assert!(
        tri.edge_count() > 6,
        "the two components must be joined by a new edge"
    );
    assert!(euler_certificate(&tri));
}

#[test]
fn every_face_but_the_outer_one_ends_up_a_triangle() {
    for (rows, cols) in [(3, 3), (2, 5)] {
        let (n, edges) = grid(rows, cols);
        let embedding = planar_embedding(n, &edges).expect("a grid is planar");
        let (tri, outer) = triangulate_embedding(&embedding);
        assert!(euler_certificate(&tri));
        let all = faces(&tri);
        let excess: i64 = (0..all.len()).map(|i| all.face(i).len() as i64 - 3).sum();
        assert_eq!(
            excess,
            outer.len() as i64 - 3,
            "only the outer face may be non-triangular"
        );
    }
}

#[test]
fn triangulating_an_already_maximal_planar_graph_leaves_edges_unchanged() {
    let edges = maximal_planar(7, 30);
    let embedding = planar_embedding(30, &edges).expect("built planar by construction");
    let (tri, _outer) = triangulate_embedding(&embedding);
    assert_eq!(
        tri.edge_count(),
        embedding.edge_count(),
        "already every face a triangle"
    );
}

#[test]
fn triangulate_embedding_is_deterministic() {
    let edges = outerplanar_fan(11);
    let embedding = planar_embedding(11, &edges).expect("outerplanar is planar");
    let (a, oa) = triangulate_embedding(&embedding);
    let (b, ob) = triangulate_embedding(&embedding);
    assert_eq!(a, b);
    assert_eq!(oa, ob);
}

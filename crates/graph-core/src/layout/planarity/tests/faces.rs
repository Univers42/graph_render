//! [`faces`]: the half-edge face trace, and the `Faces` value it returns. A `Faces` is
//! CSR-shaped exactly like an [`Embedding`], and its `len()` must be right for every
//! value the type can hold — including the one `Default` builds.
//!
//! The last test here pins down *exactly* what [`euler_certificate`] checks, which is
//! what the module doc claims: the face count of a rotation system, and nothing about
//! the rotation itself.
//!
//! [`Embedding`]: crate::layout::planarity::Embedding
//! [`euler_certificate`]: crate::layout::planarity::euler_certificate

use crate::layout::planarity::{Faces, faces, planar_embedding};

#[test]
fn a_default_faces_value_reports_no_faces_rather_than_underflowing() {
    let empty = Faces::default();
    assert_eq!(empty.len(), 0, "no face traced must read as zero, not wrap");
    assert!(empty.is_empty());
}

#[test]
fn an_edgeless_embedding_traces_no_face() {
    let embedding = planar_embedding(3, &[]).expect("isolated nodes are planar");
    let traced = faces(&embedding);
    assert_eq!((traced.len(), traced.is_empty()), (0, true));
    assert_eq!(
        traced,
        Faces::default(),
        "a graph with no edge traces the same value `Default` builds"
    );
}

#[test]
fn a_single_edge_borders_exactly_one_face_and_its_trace_is_exact() {
    let embedding = planar_embedding(2, &[(0, 1)]).expect("an edge is planar");
    let traced = faces(&embedding);
    assert_eq!(traced.len(), 1);
    // The single face walks the half-edge out and straight back.
    assert_eq!(traced.face(0), [0, 1]);
}

#[test]
fn every_half_edge_of_an_embedding_belongs_to_exactly_one_traced_face() {
    // The trace is a partition of the rotation: each half-edge is walked once, so the
    // face lengths must add up to the embedding's half-edge count and no face may run
    // off the end of a row.
    for (n, edges) in [
        (3, vec![(0, 1), (1, 2), (0, 2)]),
        (4, vec![(0, 1), (1, 2), (2, 3), (0, 3)]),
    ] {
        let embedding = planar_embedding(n, &edges).expect("planar by construction");
        let traced = faces(&embedding);
        let total: usize = (0..traced.len()).map(|i| traced.face(i).len()).sum();
        assert_eq!(total, embedding.neighbours().len(), "n={n}");
        for i in 0..traced.len() {
            for &v in traced.face(i) {
                assert!(v < n, "n={n}: face node {v} is a real node");
            }
        }
    }
}

/// The exact scope of [`euler_certificate`], and the reason the module doc is worded the
/// way it is. K5 has no genus-zero rotation at all, so *every* rotation system over its
/// ten edges has a trace whose face count is not `2 - V + E = 7`: the certificate reads
/// the face count and refuses. That is the whole of what it does — a necessary condition
/// on one number, never a check that a given rotation is the *right* one. Every planar
/// rotation of a planar graph has the same `V`, `E` and `F` and passes, however wrongly
/// ordered.
///
/// Rows are listed ascending, which is a perfectly consistent rotation system, so the
/// trace is well defined and the count is exactly what a drawing would have to give.
#[test]
fn a_rotation_system_of_a_non_planar_graph_never_hits_the_euler_face_count() {
    let mut offsets = vec![0u32];
    let mut neighbours = Vec::new();
    for v in 0..5u32 {
        neighbours.extend((0..5u32).filter(|&u| u != v));
        offsets.push(neighbours.len() as u32);
    }
    let k5 = super::super::Embedding::new(5, offsets, neighbours);
    let traced = faces(&k5);
    assert_ne!(traced.len(), 7, "K5 admits no genus-zero rotation");
    assert!(
        !super::super::euler_certificate(&k5),
        "so the certificate, which only reads the face count, must refuse it"
    );
}

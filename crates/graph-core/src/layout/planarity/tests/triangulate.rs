//! [`triangulate_embedding`]: fan-triangulates every face but the largest, leaving that
//! one as the outer boundary, joining disconnected components first.

use super::graphs::{grid, maximal_planar, outerplanar_fan, tied_longest_faces, wheel};
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

/// One largest-face case: node count, the graph, the length its largest face must have,
/// and a label naming the family it came from.
type OuterFaceCase = (u32, Vec<(u32, u32)>, usize, String);

/// The boundary handed back is the **largest** face — that is the whole contract of
/// `fully_triangulate = false`, and the packer downstream reads it as the outer ring, so
/// leaving a smaller face alone is not a cosmetic difference. For each family the largest
/// face's length is fixed by the graph, not read off the code: a grid's outer face is its
/// boundary walk, a wheel's is its rim, a maximal planar graph's is a triangle because
/// every face is one, and an outerplanar fan's is the whole cycle.
#[test]
fn the_boundary_left_alone_is_the_largest_face_of_every_family() {
    let mut cases: Vec<OuterFaceCase> = Vec::new();
    for (rows, cols) in [(2u32, 2u32), (2, 5), (3, 3), (4, 7), (7, 3)] {
        let (n, edges) = grid(rows, cols);
        let len = 2 * (rows + cols) as usize - 4;
        cases.push((n, edges, len, format!("grid {rows}x{cols}")));
    }
    for rim in [4u32, 5, 9, 20, 41] {
        let (n, edges) = wheel(rim);
        cases.push((n, edges, rim as usize, format!("wheel rim {rim}")));
    }
    for n in [6u32, 17, 40] {
        cases.push((n, maximal_planar(5, n), 3, format!("maximal planar n {n}")));
    }
    for n in [5u32, 11, 30] {
        cases.push((n, outerplanar_fan(n), n as usize, format!("fan {n}")));
    }
    for (n, edges, len, what) in cases {
        let embedding = planar_embedding(n, &edges).expect("family graph is planar");
        let (_tri, outer) = triangulate_embedding(&embedding);
        assert_eq!(outer.len(), len, "{what}: the largest face is {len} nodes");
    }
}

/// The one case in the suite where the largest face is a **tie** between two different
/// cycles — [`tied_longest_faces`]'s three faces are 6, 4 and 6 nodes — so the search has
/// to choose, and its choice has to be the deterministic one: the *first* longest face,
/// not the last, or one fixed graph would triangulate differently depending on which
/// candidate the search happened to stop on.
///
/// Both halves are read off the graph and the reference, not off this port. The two tied
/// faces are the two length-4-plus-length-2 walks, and faces are traced from node 0's
/// rotation in order, so the first traced face leaves along node 0's *first* neighbour
/// while the second tied face leaves along its second — which is why the assertion also
/// says the two candidates are not interchangeable.
#[test]
fn a_tie_for_the_largest_face_goes_to_the_first_one_traced() {
    let (n, edges) = tied_longest_faces();
    let embedding = planar_embedding(n, &edges).expect("a theta graph is planar");
    let (_tri, outer) = triangulate_embedding(&embedding);
    assert_eq!(outer.len(), 6, "the two largest faces both have 6 nodes");
    assert_eq!(outer[0], 0, "the first traced face starts at node 0");
    assert_eq!(
        outer[1],
        embedding.rotation(0)[0],
        "and leaves along node 0's *first* neighbour, not its second"
    );
    assert_ne!(outer[1], embedding.rotation(0)[1]);
}

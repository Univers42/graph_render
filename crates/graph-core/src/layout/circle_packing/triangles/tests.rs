//! The flower builder: the defensive refusals that send a graph to the fallback, the
//! boundary match against the embedding's own outer face, and the edge list the tangency
//! refinement chases.

use super::{face_key, flower, tri_edges};
use crate::layout::planarity::{Faces, faces, planar_embedding, triangulate_embedding};

/// A wheel's own edges: a hub joined to every node of a `rim`-node cycle.
fn edges_of_wheel(rim: u32) -> Vec<(u32, u32)> {
    let mut edges: Vec<(u32, u32)> = (1..=rim).map(|i| (0, i)).collect();
    edges.extend((1..=rim).map(|i| (i, if i == rim { 1 } else { i + 1 })));
    edges
}

/// A wheel's certified embedding, triangulated: `rim + 1` nodes, one hub, and every
/// interior face already a triangle. The real thing, not a hand-built stand-in.
fn triangulated_wheel(
    rim: u32,
) -> (
    u32,
    Vec<(u32, u32)>,
    crate::layout::planarity::Embedding,
    Vec<u32>,
) {
    let edges = edges_of_wheel(rim);
    let n = rim + 1;
    let embedding = planar_embedding(n, &edges).expect("a wheel is planar");
    let (triangulated, outer) = triangulate_embedding(&embedding);
    (n, edges, triangulated, outer)
}

#[test]
fn a_certified_wheel_gives_a_flower_whose_every_face_is_a_triangle() {
    let (n, _, embedding, outer) = triangulated_wheel(6);
    let built = flower(n, &embedding, &outer).expect("a wheel triangulates into a disk");
    // Every face but the outer one is a triangle, and there are as many of them as the
    // embedding has faces less the outer one — the count a triangulated disk must have
    // (more than `2V - 4` here, because the wheel's outer face needed diagonals adding).
    assert!(built.triangles.iter().all(|t| t.len() == 3));
    let traced = faces(&embedding);
    assert_eq!(
        built.triangles.len(),
        traced.len() as usize - 1,
        "one face is the outer one"
    );
    assert!(
        built.triangles.len() > n as usize - 2,
        "the outer face was triangulated"
    );
    // The boundary is exactly the outer face's nodes, in ascending dense index.
    let mut expected: Vec<u32> = outer.clone();
    expected.sort_unstable();
    expected.dedup();
    assert_eq!(built.boundary_list, expected);
    for &v in &built.boundary_list {
        assert!(built.boundary[v as usize], "boundary flag {v}");
    }
    // Every one of the graph's own edges is a triangle edge, since triangulation only
    // adds: that is what makes every original edge reach tangency.
    let tri = tri_edges(&built.triangles);
    for (u, v) in &edges_of_wheel(6) {
        let (lo, hi) = if u < v { (*u, *v) } else { (*v, *u) };
        assert!(
            tri.contains(&(lo, hi)),
            "graph edge ({lo}, {hi}) is not a triangle edge"
        );
    }
}

#[test]
fn an_outer_face_that_matches_no_traced_face_refuses() {
    // The defensive refusal: `outer_index` returns `None` when nothing equals the
    // embedding's own boundary, and the caller falls back rather than pack a disk whose
    // boundary it cannot name.
    let (n, _, embedding, _) = triangulated_wheel(6);
    let bogus = vec![n - 1, 0, 1];
    assert!(
        flower(n, &embedding, &bogus).is_none(),
        "a boundary nobody traced"
    );
}

#[test]
fn an_embedding_with_no_triangular_faces_refuses() {
    // A single edge: traced, but there is no face to triangulate, so there is no disk.
    let embedding = planar_embedding(2, &[(0, 1)]).expect("planar");
    let (triangulated, outer) = triangulate_embedding(&embedding);
    assert!(flower(2, &triangulated, &outer).is_none());
}

#[test]
fn a_face_key_is_the_lowest_index_first_and_compares_across_rotations() {
    // `_face_key`: rotate a cyclic sequence so its lowest index leads, so the same cycle
    // traced from two different starts compares equal.
    assert_eq!(face_key(&[2, 3, 0, 1]), vec![0, 1, 2, 3]);
    assert_eq!(face_key(&[1, 2, 3, 0]), face_key(&[3, 0, 1, 2]));
    assert_eq!(face_key(&[]), Vec::<u32>::new());
    assert_eq!(face_key(&[7]), vec![7]);
    // Different cycles do not collide.
    assert_ne!(face_key(&[0, 1, 2]), face_key(&[0, 2, 1]));
}

#[test]
fn every_triangulation_edge_appears_once_as_an_ascending_pair() {
    // Two triangles sharing an edge: the shared edge appears in both, and the list keeps
    // it once, so the refinement never chases it twice.
    let triangles = [[0, 1, 2], [1, 2, 3]];
    let edges = tri_edges(&triangles);
    assert_eq!(edges.len(), 5, "six half-edges, one shared: {edges:?}");
    // Ascending within every pair, and ascending across the list.
    for w in edges.windows(2) {
        assert!(w[0] < w[1], "sorted and deduplicated: {edges:?}");
        assert!(w[0].0 < w[0].1, "ascending within a pair: {w:?}");
    }
    for expected in [(0, 1), (0, 2), (1, 2), (1, 3), (2, 3)] {
        assert!(edges.contains(&expected), "missing {expected:?}");
    }
    // The same edge written the other way round keys the same, which is what the
    // refinement's target lookup and the fallback's `edge_key` both assume.
    let reversed = tri_edges(&[[0, 2, 1], [2, 1, 3]]);
    assert_eq!(reversed, edges);
    // An empty triangulation gives an empty list, and the refinement leaves the packing
    // alone rather than dividing by an empty target list.
    assert!(tri_edges(&[]).is_empty());
}

#[test]
fn the_flower_is_independent_of_the_outer_faces_rotation_direction() {
    // SciGraphs compares the outer face in both rotations (`circle_packing.py:82`), and
    // this port does too, so a reversed boundary finds the same disk.
    let (n, _, embedding, outer) = triangulated_wheel(6);
    let mut reversed = outer.clone();
    reversed.reverse();
    let forward = flower(n, &embedding, &outer).expect("matches");
    let backward = flower(n, &embedding, &reversed).expect("also matches, reversed");
    assert_eq!(forward.boundary_list, backward.boundary_list);
    assert_eq!(forward.triangles, backward.triangles);
}

#[test]
fn a_traced_embedding_of_a_real_disk_faces_itself_consistently() {
    // The planarity certificate's own output, traced: the half-edge traversal must return
    // to where it started, or the flower built from it is not a disk at all.
    let (_, _, embedding, _) = triangulated_wheel(6);
    let traced: Faces = faces(&embedding);
    let mut total = 0;
    for i in 0..traced.len() {
        let f = traced.face(i);
        assert!(f.len() >= 3, "face {i} is too small to be a face: {f:?}");
        total += f.len();
    }
    // Every half-edge of the embedding is in exactly one face, so the face lengths add up
    // to twice the edge count — the same Euler bookkeeping the certificate does.
    assert_eq!(
        total as u32,
        2 * embedding.edge_count(),
        "each half-edge walked once"
    );
}

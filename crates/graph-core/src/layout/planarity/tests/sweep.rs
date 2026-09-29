//! A wide sweep over families of planar graphs, asserting the properties a *correct*
//! combinatorial embedding has and that a wrong one cannot fake.
//!
//! The existing tests mostly assert "planar, and the rows are the right neighbours" —
//! which any rotation passes, since it is checked rotation by rotation and never
//! against the graph. This file adds the rotation-sensitive half:
//!
//! - **uniqueness of the embedding.** A 3-connected planar graph has exactly two planar
//!   rotation systems (one and its mirror), so its face set is *forced*. A maximal planar
//!   graph (an Apollonian network) is 3-connected and triangulates the sphere, so
//!   **every** face of its embedding is a triangle and there are exactly `2n - 4` of
//!   them; a wheel is a cone over a cycle, so its faces are `rim` triangles and one face
//!   of length `rim`; a grid is a rectangular lattice, so its faces are the
//!   `(rows - 1) * (cols - 1)` cells plus one boundary walk of `2 * (rows + cols) - 4`.
//!   A mutation that permutes one row's rotation, resolves a side wrongly, or picks the
//!   wrong reference changes the face set, and these counts are what catch it. Every
//!   number is derived from the graph; none of them is a golden value.
//! - **order independence**, over the whole sweep rather than one graph: reversing the
//!   input order, rotating it and flipping every edge's endpoints must not move a single
//!   rotation (D1-D10: dense index, never input or hash order).

use super::graphs::{apexed_path, cycle, grid, maximal_planar, outerplanar_fan, path, star, wheel};
use crate::layout::planarity::{euler_certificate, faces, planar_embedding};

/// Every face length of `edges`'s embedding, ascending — the shape of its rotation
/// system. Panics with the graph in the message if it is not embedded at all.
fn face_lengths(n: u32, edges: &[(u32, u32)]) -> Vec<usize> {
    let embedding = planar_embedding(n, edges)
        .unwrap_or_else(|| panic!("expected planar: n={n} edges={edges:?}"));
    assert!(euler_certificate(&embedding), "n={n} edges={edges:?}");
    let traced = faces(&embedding);
    let mut lens: Vec<usize> = (0..traced.len()).map(|i| traced.face(i).len()).collect();
    lens.sort_unstable();
    lens
}

/// `count` faces of `len` nodes, and nothing else.
fn only(count: usize, len: usize, lens: &[usize]) -> bool {
    lens.len() == count && lens.iter().all(|&l| l == len)
}

/// A maximal planar graph is 3-connected, so its planar embedding is unique up to
/// reflection: every face is a triangle, and `V - E + F = 2` with `E = 3V - 6` pins
/// `F = 2V - 4`. Both halves are load-bearing — a rotation that is merely *a* valid
/// rotation system, but not the planar one, has a different face set.
#[test]
fn a_maximal_planar_graphs_embedding_is_all_triangles_and_nothing_else() {
    for seed in 0..8u32 {
        for n in [4u32, 5, 8, 13, 21, 34] {
            let edges = maximal_planar(seed, n);
            let lens = face_lengths(n, &edges);
            assert!(
                only(2 * n as usize - 4, 3, &lens),
                "seed {seed} n {n}: every face of a sphere triangulation is a triangle, \
                 and there are 2n - 4 = {} of them, got {lens:?}",
                2 * n as usize - 4
            );
        }
    }
}

/// Same argument for the cone over a cycle. A wheel is 3-connected, so its planar
/// embedding is unique up to reflection, and in it the hub sits *inside* the rim: the
/// `rim` wedges between consecutive spokes are triangles, and the outer face is the rim
/// cycle itself, `rim` nodes, not `rim + 1`. `rim = 3` is deliberately absent: that wheel
/// is K4, whose embedding is the tetrahedron, so all four of its faces are triangles and
/// there is no separate outer face to speak of.
#[test]
fn a_wheels_embedding_is_its_triangles_plus_one_outer_face() {
    for rim in [4u32, 5, 9, 20, 41] {
        let (n, edges) = wheel(rim);
        let lens = face_lengths(n, &edges);
        assert_eq!(lens.len(), rim as usize + 1, "rim {rim}: {lens:?}");
        assert!(
            lens[..rim as usize].iter().all(|&l| l == 3),
            "rim {rim}: the wedges between spokes are triangles, got {lens:?}"
        );
        assert_eq!(lens[rim as usize], rim as usize, "rim {rim}: {lens:?}");
    }
}

/// Same for a rectangular grid, where the faces are the cells plus the boundary walk.
#[test]
fn a_grids_embedding_is_its_cells_plus_one_outer_face() {
    for (rows, cols) in [(2u32, 2u32), (2, 5), (3, 3), (4, 7), (7, 4), (2, 30)] {
        let (n, edges) = grid(rows, cols);
        let lens = face_lengths(n, &edges);
        let cells = (rows * cols - rows - cols + 1) as usize;
        assert_eq!(lens.len(), cells + 1, "{rows}x{cols}: {lens:?}");
        assert!(
            lens[..cells].iter().all(|&l| l == 4),
            "{rows}x{cols}: the cell interiors are squares, got {lens:?}"
        );
        assert_eq!(
            lens[cells],
            2 * (rows + cols) as usize - 4,
            "{rows}x{cols}: the outer face is the boundary walk, got {lens:?}"
        );
    }
}

/// The rotation is dense-index determined, so the order the edges arrive in cannot reach
/// it: reversing the list, rotating it and flipping every edge's endpoints all describe
/// the same simple graph and must give the byte-identical embedding. Swept over the whole
/// family list rather than one graph, because a mutation can be invisible on any single
/// one of them.
#[test]
fn no_row_of_the_sweep_moves_when_the_input_order_or_endpoints_do() {
    for (n, edges) in sweep_families() {
        let straight = planar_embedding(n, &edges).expect("family graph is planar");
        let mut flipped: Vec<(u32, u32)> = edges.iter().map(|&(a, b)| (b, a)).collect();
        let third = flipped.len() / 3;
        flipped.reverse();
        flipped.rotate_left(third);
        assert_eq!(
            straight,
            planar_embedding(n, &flipped).expect("same simple graph"),
            "n={n} edges={edges:?}: input order and endpoint order must not be visible"
        );
    }
}

/// The same sweep for the properties that survive every relabelling: a correct embedding
/// is *some* planar rotation system, so it is deterministic, it satisfies Euler, and its
/// rows carry two half-edges per **simple** edge. The edge count is re-derived rather
/// than taken from `edges.len()`, because builders here legitimately hand over the same
/// pair twice (`cycle(2)` does) and the input length is not the graph's.
#[test]
fn the_whole_sweep_embeds_certifies_and_repeats() {
    for (n, edges) in sweep_families() {
        let a = planar_embedding(n, &edges).expect("family graph is planar");
        let b = planar_embedding(n, &edges).expect("family graph is planar");
        assert_eq!(a, b, "n={n}: must be deterministic");
        assert!(euler_certificate(&a), "n={n} edges={edges:?}");
        assert_eq!(a.node_count(), n);
        let mut simple: Vec<(u32, u32)> = edges
            .iter()
            .map(|&(a, b)| if a < b { (a, b) } else { (b, a) })
            .collect();
        simple.sort_unstable();
        simple.dedup();
        assert_eq!(
            a.neighbours().len() as u32,
            2 * simple.len() as u32,
            "n={n} edges={edges:?}: two half-edges per simple edge"
        );
    }
}

/// The planar families every sweep above runs over, in a fixed order: the tree shapes,
/// the grid shape, the hub-and-cycle shapes, the outerplanar fan, the two-apex
/// suspension, the edge-maximal Apollonian networks, a disconnected pair, and the
/// two-theta graph that ties the longest-face search in
/// [`triangulate_embedding`](crate::layout::planarity::triangulate_embedding).
fn sweep_families() -> Vec<(u32, Vec<(u32, u32)>)> {
    let mut out: Vec<(u32, Vec<(u32, u32)>)> = Vec::new();
    for n in [2u32, 5, 9, 20, 64] {
        out.push((n, path(n)));
        out.push((n, star(n)));
        out.push((n, cycle(n)));
        out.push((n, outerplanar_fan(n)));
    }
    for (rows, cols) in [(2u32, 2u32), (3, 4), (5, 5)] {
        out.push(grid(rows, cols));
    }
    for rim in [3u32, 4, 7, 15] {
        out.push(wheel(rim));
    }
    for span in [3u32, 4, 6, 12] {
        out.push(apexed_path(span));
    }
    for seed in 0..4u32 {
        for n in [4u32, 9, 17] {
            out.push((n, maximal_planar(seed, n)));
        }
    }
    // Two components at once: the certificate has to account for the second one.
    out.push((8, {
        let mut e = cycle(4);
        e.extend(cycle(4).into_iter().map(|(a, b)| (a + 4, b + 4)));
        e
    }));
    out.push(super::graphs::tied_longest_faces());
    out
}

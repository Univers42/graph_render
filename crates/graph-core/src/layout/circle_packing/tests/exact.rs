//! The exact (planar) path: every graph edge tangent within a stated `f64` tolerance, no
//! two circles overlap beyond it, and no note is recorded — `docs/decisions/
//! planarity-fallback.md`'s three exact-path requirements. Tangency is measured against
//! the *original* graph's edges, not the triangulation's own (`circle_packing.py:
//! 355-359, :372`: "report against the original graph's edges, not the triangulated ones").

use super::support::{cycle, grid, topology, wheel};
use crate::layout::circle_packing::run;

/// Loose enough to absorb `f32` rounding on top of the `1e-9` `f64` solver tolerance and
/// the gradient-descent refinement's own `1e-9` stopping error; tight enough that a
/// packing which never tried to reach tangency would still fail it.
const TANGENCY_TOLERANCE: f32 = 1e-3;

struct Circles {
    x: Vec<f32>,
    y: Vec<f32>,
    r: Vec<f32>,
}

fn circles(geometry: &crate::layout::Geometry) -> Circles {
    let graph_contract::geometry::NodeGeometry::Circle { x, y, r } = &geometry.nodes else {
        panic!("circle packing always emits Circle geometry");
    };
    Circles {
        x: x.clone(),
        y: y.clone(),
        r: r.clone(),
    }
}

fn distance(c: &Circles, i: usize, j: usize) -> f32 {
    let (dx, dy) = (c.x[i] - c.x[j], c.y[i] - c.y[j]);
    (dx * dx + dy * dy).sqrt()
}

/// The worst `|distance - (r_i + r_j)|` over `edges`, and whether every one is within
/// [`TANGENCY_TOLERANCE`].
fn max_tangency_error(c: &Circles, edges: &[(u32, u32)]) -> f32 {
    edges
        .iter()
        .filter(|&&(u, v)| u != v)
        .map(|&(u, v)| (u as usize, v as usize))
        .map(|(u, v)| (distance(c, u, v) - (c.r[u] + c.r[v])).abs())
        .fold(0.0, f32::max)
}

fn assert_exact(n: u32, edges: &[(u32, u32)]) -> Circles {
    let geometry = run(&topology(n, edges)).expect("runs");
    assert!(geometry.notes.is_empty(), "exact path carries no note");
    let c = circles(&geometry);
    let error = max_tangency_error(&c, edges);
    assert!(error < TANGENCY_TOLERANCE, "max tangency error {error}");
    for i in 0..c.x.len() {
        for j in (i + 1)..c.x.len() {
            let gap = distance(&c, i, j) - (c.r[i] + c.r[j]);
            assert!(
                gap > -TANGENCY_TOLERANCE,
                "circles {i},{j} overlap by {gap}"
            );
        }
    }
    c
}

#[test]
fn a_triangle_packs_into_three_mutually_tangent_circles() {
    assert_exact(3, &[(0, 1), (1, 2), (0, 2)]);
}

#[test]
fn a_square_with_one_diagonal_packs_exactly() {
    assert_exact(4, &[(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)]);
}

#[test]
fn a_bare_cycle_with_no_chords_still_packs_exactly() {
    assert_exact(6, &cycle(6));
}

#[test]
fn a_wheel_packs_exactly() {
    let (n, edges) = wheel(6);
    assert_exact(n, &edges);
}

#[test]
fn a_sparse_grid_with_no_triangles_of_its_own_still_packs_exactly() {
    let (n, edges) = grid(3, 4);
    assert_exact(n, &edges);
}

#[test]
fn isolated_nodes_with_no_edges_at_all_still_pack() {
    let c = assert_exact(4, &[]);
    assert_eq!(c.x.len(), 4);
}

/// Reports the worst tangency error this port actually measures across every planar
/// fixture above, so the number is on record rather than only "under the tolerance".
#[test]
fn the_worst_observed_tangency_error_is_reported() {
    let cases: [(u32, Vec<(u32, u32)>); 5] = [
        (3, vec![(0, 1), (1, 2), (0, 2)]),
        (4, vec![(0, 1), (1, 2), (2, 3), (3, 0), (0, 2)]),
        (6, cycle(6)),
        (7, wheel(6).1),
        (12, grid(3, 4).1),
    ];
    let mut worst = 0.0_f32;
    for (n, edges) in &cases {
        let geometry = run(&topology(*n, edges)).expect("runs");
        worst = worst.max(max_tangency_error(&circles(&geometry), edges));
    }
    println!("circle_packing exact path: worst observed tangency error = {worst:e}");
    assert!(
        worst < TANGENCY_TOLERANCE,
        "worst observed tangency error {worst}"
    );
}

#[test]
fn a_self_loop_and_a_duplicate_edge_are_reduced_away_like_scigraphs_simple() {
    let plain = assert_exact(3, &[(0, 1), (1, 2), (0, 2)]);
    let messy = assert_exact(3, &[(0, 1), (1, 2), (0, 2), (0, 1), (2, 2)]);
    assert_eq!(plain.r, messy.r, "same simple graph once reduced");
}

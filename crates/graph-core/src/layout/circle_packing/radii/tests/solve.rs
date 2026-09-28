//! The Jacobi sweep: the two refusals, the reported error, the free mask, and the mean
//! renormalisation that keeps the radii comparable from sweep to sweep.

use super::super::packing_aims;
use super::super::{Coeffs, solve_packing_radii};
use super::{TOLERANCE, fan, packing_angle};
use core::f64::consts::PI;

#[test]
fn a_flower_with_no_corners_solves_to_unit_radii() {
    // The empty-disk refusal: no corners means there is nothing to sum angles over, and
    // the reference returns the initial unit radii with no error. The aims here are the
    // opposite of what a corner would close on, so a guard that had been narrowed to
    // `&&` would run the solve, report a large error and move every free radius — this
    // is the case that tells the two halves of the `||` apart.
    for aims in [vec![0.0; 3], vec![1.0, 1.0, 1.0]] {
        for free in [vec![], vec![0], vec![0, 1, 2]] {
            let solved = solve_packing_radii(&[vec![], vec![], vec![]], &aims, &free, 10);
            assert_eq!(solved.radii, vec![1.0; 3], "aims {aims:?} free {free:?}");
            assert_eq!(solved.max_error, 0.0, "free {free:?}: no error to report");
        }
    }
}

#[test]
fn a_solve_with_no_free_vertex_solves_to_unit_radii() {
    // Every vertex pinned: the sweep has nothing to update, so the radii hold at 1.
    let (at, boundary, _) = fan(5);
    let aims = packing_aims(&at, &boundary);
    let solved = solve_packing_radii(&at, &aims, &[], 10);
    assert_eq!(solved.radii, vec![1.0; 5]);
    assert_eq!(solved.max_error, 0.0);
}

#[test]
fn a_solve_reports_the_worst_free_error_it_is_left_with() {
    // The reported error is measured *before* each sweep, from the radii the previous
    // sweep left, so it is never negative and it is the error of the returned radii.
    let (at, boundary, free) = fan(5);
    let aims = packing_aims(&at, &boundary);
    let one = solve_packing_radii(&at, &aims, &free, 1);
    let many = solve_packing_radii(&at, &aims, &free, 40);
    assert!(one.max_error >= 0.0 && many.max_error >= 0.0);
    assert!(
        one.max_error >= TOLERANCE,
        "one sweep is far from converged"
    );
    // The sweeps do run: the radii a longer budget returns are not the ones a single
    // sweep produced.
    assert_ne!(many.radii, one.radii, "the budget must reach the sweeps");
}

#[test]
fn a_converged_solve_stops_on_the_tolerance_itself() {
    // The loop's own convergence test is `< TOLERANCE`, so a solve that reaches an error
    // of exactly the tolerance keeps sweeping and a `<=` would not. Vertex 0 is free and
    // carries no corner, so its angle sum is exactly 0 and an aim of exactly TOLERANCE
    // puts its error precisely on the threshold; the other two carry the corners that
    // make the flower non-empty, and are aimed at what their own corner closes on.
    let at = vec![vec![], vec![(0, 2)], vec![(0, 1)]];
    let corner = packing_angle(1.0, 1.0, 1.0);
    let aims = [TOLERANCE, corner, corner];
    let solved = solve_packing_radii(&at, &aims, &[0, 1, 2], 1);
    assert_eq!(
        solved.max_error.to_bits(),
        TOLERANCE.to_bits(),
        "the free vertex with no corners is the whole error"
    );
    // With `<=` that solve would have broken before sweeping; with `<` it swept, so the
    // radii it returns are not the untouched unit ones.
    assert_ne!(solved.radii[0], 1.0, "the sweep ran");
}

#[test]
fn a_generous_budget_converges_on_a_real_disk() {
    // The whole point of the solver: with the reference's own `iterations * 20` sweeps a
    // fan converges, and what it converges to is finite, positive and tame.
    // A hand-built fan is not always embeddable flat (a two-corner interior vertex aims
    // at two straight angles), so this uses a real certified disk: a triangulated wheel,
    // which is what the exact path actually feeds the solver.
    let rim = 6u32;
    let n = rim + 1;
    let mut edges: Vec<(u32, u32)> = (1..=rim).map(|i| (0, i)).collect();
    edges.extend((1..=rim).map(|i| (i, if i == rim { 1 } else { i + 1 })));
    let embedding = crate::layout::planarity::planar_embedding(n, &edges).expect("planar");
    let (triangulated, outer) = crate::layout::planarity::triangulate_embedding(&embedding);
    let flower = crate::layout::circle_packing::triangles::flower(n, &triangulated, &outer)
        .expect("a wheel triangulates into a disk");
    let aims = packing_aims(&flower.at, &flower.boundary_list);
    let free: Vec<u32> = (0..n)
        .filter(|&v| !flower.at[v as usize].is_empty())
        .collect();
    let converged = solve_packing_radii(&flower.at, &aims, &free, 5000);
    assert!(
        converged.max_error < TOLERANCE,
        "a wheel should converge: {}",
        converged.max_error
    );
    assert!(
        converged.radii.iter().all(|r| r.is_finite() && *r > 0.0),
        "{:?}",
        converged.radii
    );
    // The reference divides the whole array by its mean each sweep
    // (`circle_packing.py:173-175`), so the radii of a converged disk sit around 1
    // rather than drifting off.
    let mean: f64 = converged.radii.iter().sum::<f64>() / converged.radii.len() as f64;
    assert!(
        (mean - 1.0).abs() < 1e-3,
        "mean {mean} should sit at the normalisation point"
    );
}

#[test]
fn a_solve_that_cannot_converge_still_returns_finite_radii() {
    // Aims no triangle can close on: the sweeps run to the cap and the answer is
    // whatever they reached, which must still be finite and positive — the caller
    // (`is_embedded`) is what decides this is not exact, not a NaN leaking out of here.
    let (at, _, free) = fan(5);
    let aims = vec![0.1; 5];
    let solved = solve_packing_radii(&at, &aims, &free, 50);
    assert!(solved.max_error >= TOLERANCE, "{}", solved.max_error);
    assert!(
        solved.radii.iter().all(|r| r.is_finite() && *r > 0.0),
        "{:?}",
        solved.radii
    );
}

#[test]
fn a_sweep_holds_a_pinned_vertex_at_its_radius() {
    // The free mask, directly: one free vertex, two pinned. A pinned vertex keeps the
    // radius it was handed (1) *relative to the others* — the sweep's own `radii = where
    // (free, updated, radii)` never touches it — while the free one is recomputed, so the
    // three no longer share a radius. The whole array is then divided by its mean, which
    // moves every entry by the same factor and so cannot change that.
    let at = vec![vec![(1, 2)], vec![(0, 2)], vec![(0, 1)]];
    let aims = [2.0 * PI; 3];
    let solved = solve_packing_radii(&at, &aims, &[0], 5);
    assert_eq!(
        solved.radii[1].to_bits(),
        solved.radii[2].to_bits(),
        "both pinned, still equal to each other"
    );
    assert_ne!(
        solved.radii[0].to_bits(),
        solved.radii[1].to_bits(),
        "the free vertex moved off the pinned pair"
    );
    // With every vertex pinned, nothing moves at all.
    let all_pinned = solve_packing_radii(&at, &aims, &[], 5);
    assert_eq!(all_pinned.radii, vec![1.0; 3]);
}

#[test]
fn coeffs_clamp_a_vertex_with_no_corners_to_a_count_of_one() {
    // `np.maximum(np.bincount(centre), 1)`: a vertex with no corners still gets a count
    // of 1, so its `delta` divides by 1 rather than by 0.
    let corners = [(0, 1, 2), (1, 2, 0)];
    let aims = [2.0 * PI; 3];
    let free_mask = [true, true, false];
    let coeffs = Coeffs::build(&corners, &aims, &free_mask, 3);
    assert_eq!(coeffs.counts, vec![1.0, 1.0, 1.0]);
    assert_eq!(coeffs.free, free_mask.to_vec());
    for &v in &coeffs.free {
        assert!(
            coeffs.delta[v as usize].is_finite(),
            "delta {}",
            coeffs.delta[v as usize]
        );
    }
    // A vertex with two corners gets a count of 2, and a larger `delta` argument halved.
    let corners = [(0, 1, 2), (0, 2, 3)];
    let coeffs = Coeffs::build(&corners, &[PI; 4], &[true; 4], 4);
    assert_eq!(coeffs.counts, vec![2.0, 1.0, 1.0, 1.0]);
}

//! The exact path's own acceptance test, `is_embedded`, on the inputs no real solve
//! produces but a hostile or degenerate one can: a radius that is `NaN` or infinite, a
//! worst angle-sum error that is `NaN`, and a spread that overflows. `f64::max`/`f64::min`
//! ignore `NaN` (they return the other operand), so a fold over them reports the largest
//! *finite* radius and an all-`NaN` solve looks perfectly embedded — the one case where
//! the module doc's "its absence is the only trustworthy sign the packing is exact" would
//! be a lie, silently.

use super::super::tests::support::topology;
use super::super::triangles::Flower;
use super::super::{MAX_RADIUS_SPREAD, is_embedded, solve_with_fallback, spread};
use crate::layout::circle_packing::radii::{Solved, TOLERANCE};

fn solved(radii: Vec<f64>, max_error: f64) -> Solved {
    Solved { radii, max_error }
}

#[test]
fn a_converged_packing_within_the_spread_limit_is_embedded() {
    assert!(is_embedded(&solved(vec![1.0, 2.0, 4.0], 0.0)));
    // Exactly at the limit, which `<=` admits (`circle_packing.py:336`).
    assert!(is_embedded(&solved(
        vec![1.0, MAX_RADIUS_SPREAD],
        TOLERANCE / 2.0
    )));
}

#[test]
fn a_solve_that_did_not_converge_is_not_embedded() {
    assert!(!is_embedded(&solved(vec![1.0, 2.0, 4.0], TOLERANCE)));
    assert!(!is_embedded(&solved(vec![1.0, 2.0, 4.0], 1.0)));
}

#[test]
fn a_packing_that_crowds_past_the_spread_limit_is_not_embedded() {
    assert!(!is_embedded(&solved(
        vec![1.0, 1.0 + MAX_RADIUS_SPREAD * 1.000_001],
        0.0
    )));
}

#[test]
fn a_non_finite_radius_is_not_embedded() {
    // Each of these must fall through to the pinned-boundary retry, note code 3, rather
    // than being waved through as an exact packing.
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            !is_embedded(&solved(vec![1.0, 2.0, bad], 0.0)),
            "a single {bad} radius"
        );
        assert!(
            !is_embedded(&solved(vec![bad, bad, bad], 0.0)),
            "every radius {bad}"
        );
        assert!(
            !is_embedded(&solved(vec![bad, 1.0, 2.0, 4.0], 0.0)),
            "a leading {bad} radius"
        );
    }
}

#[test]
fn a_non_finite_max_error_is_not_embedded() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            !is_embedded(&solved(vec![1.0, 2.0, 4.0], bad)),
            "a max_error of {bad}"
        );
    }
}

#[test]
fn spread_reads_a_non_finite_radius_as_unbounded_not_as_the_largest_finite_one() {
    // The `f64::max`/`f64::min` fold this guards ignores `NaN` entirely, which is exactly
    // the bug: the reported spread comes out finite for an all-`NaN` packing.
    assert_eq!(spread(&[f64::NAN, f64::NAN, f64::NAN]), f64::INFINITY);
    assert_eq!(spread(&[1.0, f64::NAN, 4.0]), f64::INFINITY);
    assert_eq!(spread(&[1.0, f64::INFINITY, 4.0]), f64::INFINITY);
    // A finite spread is still the plain max/min ratio, with the reference's own 1e-300
    // floor on the denominator.
    assert_eq!(spread(&[2.0, 3.0, 4.0]), 2.0);
    assert_eq!(spread(&[1.0, 1.0]), 1.0);
    assert_eq!(spread(&[0.0, 0.0]), 0.0);
    assert_eq!(spread(&[1e-200, 1e200]), f64::INFINITY);
}

/// A disk of four vertices: 0, 1, 2 on the boundary and 3 interior, with a full flower
/// each so the solver has corners to work on. `boundary` is what decides which vertices
/// the retry pins, so a test that only looked at the returned `approximate` flag would
/// miss the filter being inverted.
fn four_vertex_disk() -> Flower {
    Flower {
        at: vec![
            vec![(1, 3), (3, 2)],
            vec![(2, 3), (3, 0)],
            vec![(0, 3), (3, 1)],
            vec![(0, 1), (1, 2), (2, 0)],
        ],
        boundary: vec![true, true, true, false],
        boundary_list: vec![0, 1, 2],
        triangles: vec![[0, 1, 3], [1, 2, 3], [2, 0, 3]],
    }
}

#[test]
fn a_non_converged_solve_retries_with_the_boundary_pinned() {
    // The end of the chain: `solve_with_fallback` retries with only the *interior*
    // vertices free and reports the result as approximate, so a non-finite or
    // non-converged solve is flagged, never shipped as exact. Built by hand because the
    // radius solver cannot be made to produce a non-finite one from a well-formed
    // triangulation.
    let flower = four_vertex_disk();
    let aims = vec![0.5, 0.5, 0.5, 0.5];
    let all_free = vec![0, 1, 2, 3];
    // Two sweeps: far too few to converge, so the retry is what is returned.
    let (retried, approximate) = solve_with_fallback(&flower, &aims, &all_free, 2);
    assert!(approximate, "a non-converged solve is approximate");
    assert_eq!(retried.len(), 4);
    assert!(
        retried.iter().all(|r| r.is_finite() && *r > 0.0),
        "{retried:?}"
    );

    // The retry pins the boundary: only vertex 3 is interior, so only it is recomputed,
    // and the three boundary radii come back from the retry as the untouched `1` its free
    // set leaves them at (renormalised by the mean, which is one factor over all four).
    // Solve the interior-only case directly and require the same answer.
    let pinned =
        crate::layout::circle_packing::radii::solve_packing_radii(&flower.at, &aims, &[3], 2);
    let ratio: Vec<f64> = retried
        .iter()
        .zip(&pinned.radii)
        .map(|(a, b)| a / b)
        .collect();
    for (i, r) in ratio.iter().enumerate() {
        assert!(
            (r - ratio[0]).abs() < 1e-9,
            "every radius scaled by one factor: {retried:?} vs {:?}",
            pinned.radii
        );
        assert!(retried[i].is_finite());
    }
    // The boundary radii are equal to each other, which is what "pinned" means here: the
    // three of them all came out of the same un-updated free set.
    assert_eq!(
        retried[0].to_bits(),
        retried[1].to_bits(),
        "the pinned boundary radii are equal"
    );
    assert_eq!(retried[1].to_bits(), retried[2].to_bits());
    assert_ne!(
        retried[3].to_bits(),
        retried[0].to_bits(),
        "the interior one moved"
    );
}

#[test]
fn a_degenerate_all_boundary_flower_still_comes_back_flagged() {
    // Every vertex on the boundary, so the retry's free set is empty: nothing is
    // recomputed and the answer is whatever the first solve left. It must still be finite,
    // and still flagged.
    let flower = Flower {
        at: vec![vec![(1, 2)], vec![(2, 0)], vec![(0, 1)]],
        boundary: vec![true, true, true],
        boundary_list: vec![0, 1, 2],
        triangles: vec![[0, 1, 2]],
    };
    let aims = vec![0.5, 0.5, 0.5];
    let all_free = vec![0, 1, 2];
    let (radii, approximate) = solve_with_fallback(&flower, &aims, &all_free, 2);
    assert!(approximate, "a non-converged solve is approximate");
    assert_eq!(radii.len(), 3);
    assert!(radii.iter().all(|r| r.is_finite() && *r > 0.0), "{radii:?}");
}

#[test]
fn a_disconnected_planar_graph_is_joined_into_one_disk_and_packs_exactly() {
    // `triangulate_embedding` joins every component with a bare edge before tracing, so
    // two disjoint triangles still become one triangulated disk and the walk reaches all
    // six vertices. The refusal that guards `placed.iter().any(|&p| !p)` is therefore
    // defensive only — pinned here so the "it is unreachable" note stays true.
    let edges = [(0, 1), (1, 2), (0, 2), (3, 4), (4, 5), (3, 5)];
    let geometry = crate::layout::circle_packing::run(&topology(6, &edges)).expect("runs");
    assert!(geometry.notes.is_empty(), "one disk, so the exact path");
    let graph_contract::geometry::NodeGeometry::Circle { x, y, r } = &geometry.nodes else {
        panic!("circle packing always emits Circle geometry");
    };
    assert_eq!(x.len(), 6);
    assert!(x.iter().chain(y).chain(r).all(|v| v.is_finite()));
}

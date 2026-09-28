//! The angle and aim half of the radius solver, on inputs chosen at its boundaries: the
//! degenerate triangle, the boundary vertex with no triangles of its own, and the bisected
//! share whose tie decides the answer in its last bits. The sweep itself is in [`solve`].
//!
//! [`solve`]: solve

mod solve;

use super::{TOLERANCE, bisect_share, packing_aims, packing_angle};
use core::f64::consts::PI;

fn all_free(n: usize) -> Vec<u32> {
    (0..n as u32).collect()
}

/// A triangulated disk's flowers for a triangle fan around vertex 0 — the triangles
/// `(0, i, i + 1)` for `i` in `1..n-1` — whose outer face is `0 -> n - 1 -> 1`, so the
/// boundary is exactly `{0, 1, n - 1}` and every other vertex is interior with a full
/// flower. Needs `n >= 4`.
type Fan = (Vec<Vec<(u32, u32)>>, Vec<u32>, Vec<u32>);

fn fan(n: usize) -> Fan {
    let mut at = vec![Vec::new(); n];
    for i in 1..n - 1 {
        at[0].push((i as u32, (i + 1) as u32));
        at[i].push((0, (i + 1) as u32));
        at[i + 1].push((0, i as u32));
    }
    let boundary: Vec<u32> = vec![0, 1, (n - 1) as u32];
    (at, boundary, all_free(n))
}

#[test]
fn a_packing_angle_is_the_corner_of_three_mutually_tangent_circles() {
    // Three equal circles: the corner is 60 degrees by symmetry, and the formula has to
    // find it from the radii alone. (`acos(0.5)`, so equal to pi/3 only to a rounding
    // step — the reference computes it the same way.)
    assert!((packing_angle(1.0, 1.0, 1.0) - PI / 3.0).abs() < 1e-15);
    // A tiny circle wedged into the crevice between two equal big ones: it touches both
    // almost where they touch each other, so the corner at `i` closes to nothing.
    assert!(
        packing_angle(1.0, 1.0, 1e-6) < 1e-2,
        "{}",
        packing_angle(1.0, 1.0, 1e-6)
    );
    // A huge one, by contrast, opens the same corner to a right angle: the dominant term
    // is `(r_i + r_j)^2 / (2 (r_i + r_j)(r_i + r_k))`, whose cosine goes to 0.
    assert!(
        (packing_angle(1.0, 1.0, 1e6) - PI / 2.0).abs() < 1e-3,
        "{}",
        packing_angle(1.0, 1.0, 1e6)
    );
    // The corner is the angle *opposite* the third circle, so the three angles of one
    // triangle add up to pi: the flowers of a triangulated disk rely on exactly that
    // (discrete Gauss–Bonnet is what `packing_aims` solves for).
    let (a, b, c) = (2.0, 3.0, 4.0);
    let total = packing_angle(a, b, c) + packing_angle(b, a, c) + packing_angle(c, a, b);
    assert!((total - PI).abs() < 1e-9, "triangle angles sum to {total}");
}

#[test]
fn a_degenerate_triangle_falls_back_to_sixty_degrees() {
    // The denominator `2 * (r_i + r_j) * (r_i + r_k)` underflows. Both sides of the
    // guard's own 1e-12 threshold are pinned, because the two branches disagree by nearly
    // 2.1 radians there — the fallback is not a rounding of the same answer.
    //
    // Exactly at the threshold: `2 * 1e-7 * 5e-6` is 1e-12, and the reference takes the
    // acos branch on `<`, so the answer is the real angle (nearly pi here).
    let at_threshold = packing_angle(0.0, 1e-7, 5e-6);
    assert!(
        at_threshold > 3.0,
        "at the guard's own threshold the acos branch runs: {at_threshold}"
    );
    // A decade below it, the centres *would* coincide and the answer is pi/3. A guard
    // that fired a decade early would take the acos branch here instead and answer very
    // nearly pi, so this is the case that pins the threshold itself.
    assert_eq!(
        packing_angle(1e-30, 1e-7, 1e-6),
        PI / 3.0,
        "2 * 1e-7 * 1e-6 = 2e-13"
    );
    // Well below it, likewise.
    assert_eq!(packing_angle(0.0, 1e-7, 1e-7), PI / 3.0);
    assert_eq!(packing_angle(0.0, 0.0, 0.0), PI / 3.0);
}

#[test]
fn every_interior_vertex_aims_at_a_full_turn() {
    // Interior vertices get the full `2pi`; only the boundary ones are given a share of
    // the convex-polygon total, and only they can fall below it.
    let (at, boundary, _) = fan(6);
    let aims = packing_aims(&at, &boundary);
    assert_eq!(aims.len(), 6);
    for v in 0..6u32 {
        if boundary.contains(&v) {
            assert!(aims[v as usize] < 2.0 * PI, "boundary {v} should be shared");
        } else {
            assert_eq!(
                aims[v as usize].to_bits(),
                (2.0 * PI).to_bits(),
                "interior {v} keeps a full turn"
            );
        }
    }
}

#[test]
fn a_boundary_fewer_than_three_vertices_keeps_the_full_turn_everywhere() {
    // One or two boundary vertices: `(B - 2) * pi` is zero or negative, so there is no
    // boundary share to hand out and every vertex stays at 2pi.
    let (at, _, _) = fan(5);
    for boundary in [vec![], vec![0], vec![0, 1]] {
        let aims = packing_aims(&at, &boundary);
        assert!(
            aims.iter().all(|&a| a.to_bits() == (2.0 * PI).to_bits()),
            "boundary {boundary:?}: {aims:?}"
        );
    }
}

#[test]
fn boundary_aims_are_capped_at_pi_and_sum_to_the_convex_polygon_total() {
    // A disk whose boundary vertices have very unequal triangle counts: discrete
    // Gauss–Bonnet says the capped shares must add up to `(B - 2) * pi` exactly, so a
    // bisection that stopped early or used the wrong comparison would not close.
    let mut at = vec![Vec::new(); 5];
    // Vertex 0: a hub with many triangles. Vertex 4: one triangle. The rest: two.
    at[0] = (1..5).map(|i| (i, i + 1)).collect();
    at[1] = vec![(0, 2), (0, 4)];
    at[2] = vec![(0, 1), (0, 3)];
    at[3] = vec![(0, 2), (0, 4)];
    at[4] = vec![(0, 3)];
    let boundary = vec![0, 1, 2, 3, 4];
    let aims = packing_aims(&at, &boundary);
    for &v in &boundary {
        assert!(
            aims[v as usize] <= PI,
            "vertex {v} over pi: {}",
            aims[v as usize]
        );
    }
    let total: f64 = boundary.iter().map(|&v| aims[v as usize]).sum();
    let want = (boundary.len() as f64 - 2.0) * PI;
    assert!((total - want).abs() < 1e-9, "sum {total} against {want}");
    // A vertex with more triangles gets a proportionally larger share.
    assert!(aims[0] > aims[4], "{:?}", aims);
}

#[test]
fn a_boundary_vertex_with_no_triangles_of_its_own_still_gets_a_share() {
    // `sum(counts) == 0` is a separate refusal from `B < 3`: with triangles missing the
    // bisection has nothing to scale, and the reference returns the untouched 2pi aims.
    let at = vec![vec![], vec![], vec![]];
    let aims = packing_aims(&at, &[0, 1, 2]);
    assert!(
        aims.iter().all(|&a| a.to_bits() == (2.0 * PI).to_bits()),
        "{aims:?}"
    );
}

#[test]
fn a_bisection_tie_goes_to_the_upper_half() {
    // `sum < total` decides which half of the bracket a bisection keeps, and a tie is a
    // real case: with these counts and this total, a midpoint's capped sum lands exactly
    // on the target, and the two branches then keep opposite halves — which is visible in
    // the last bits of the answer. The reference takes the `else` (upper) half on a tie
    // (`circle_packing.py:124-126`), so this pins `<` rather than `<=`.
    let counts = [7.0, 5.0, 6.0, 5.0, 3.0, 9.0];
    let total = 18.0;
    let capped = |c: f64| -> f64 { counts.iter().map(|&k| (c * k).min(PI)).sum() };
    let bisect = |tie_lo: bool| {
        let (mut lo, mut hi) = (0.0_f64, PI);
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if capped(mid) < total || (tie_lo && capped(mid) == total) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    };
    // The case must actually contain a tied midpoint, or it proves nothing.
    let (mut lo, mut hi) = (0.0_f64, PI);
    let mut tied = false;
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if capped(mid) == total {
            tied = true;
            break;
        }
        if capped(mid) < total {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    assert!(tied, "this case must have a tied midpoint");
    // And the tie decided the outcome: the two branches land on different bits.
    assert_ne!(bisect(true).to_bits(), bisect(false).to_bits());
    assert_eq!(
        bisect_share(&counts, total).to_bits(),
        bisect(false).to_bits()
    );
}

#[test]
fn a_bisected_share_is_inside_its_own_bracket() {
    // 60 rounds of bisection over [0, pi] converge to the multiplier whose capped sum
    // matches the total; the bracket must stay ordered, and the answer must be the one
    // that closes, not merely some point in range.
    let counts = [1.0, 2.0, 3.0, 7.0];
    let total: f64 = counts.iter().sum::<f64>() * 0.5;
    let share = bisect_share(&counts, total);
    assert!((0.0..=PI).contains(&share), "{share}");
    let got: f64 = counts.iter().map(|&k| (share * k).min(PI)).sum();
    assert!((got - total).abs() < 1e-12, "sum {got} against {total}");
    // Equal counts: the share is the total divided by them.
    let equal = bisect_share(&[2.0, 2.0, 2.0], 3.0);
    assert!((equal - 0.5).abs() < 1e-9, "{equal}");
    // And the tolerance the solver is built on is the reference's own.
    assert_eq!(TOLERANCE, 1e-9, "SciGraphs' own `tolerance=1e-9`");
}

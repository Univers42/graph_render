//! The placement walk: the seed triangle, the "measured `a - b`" rule that keeps rounding
//! drift from breaking closure, and the two degeneracy guards.

use super::super::{Walk, lay_out_packing};

#[test]
fn the_placement_walk_reaches_every_vertex_of_a_triangulated_fan() {
    // A fan of three triangles around vertex 1, wound consistently (the winding the
    // traced embedding gives them, so every edge's reverse half-edge is the next
    // triangle's own): five vertices, one connected triangulated disk, so the walk must
    // place every one.
    let triangles = [[1, 0, 2], [1, 2, 3], [1, 3, 4]];
    let radii = [1.0, 1.0, 1.0, 1.0, 1.0];
    let (positions, placed) = lay_out_packing(&triangles, &radii, 5);
    assert!(placed.iter().all(|&p| p), "every vertex placed: {placed:?}");
    // Triangle 0 is [1, 0, 2], so it is seeded exactly: its `a0` (node 1) at the origin,
    // its `b0` (node 0) on the +x axis at the tangency distance.
    assert_eq!(positions[1], (0.0, 0.0));
    assert_eq!(positions[0], (2.0, 0.0));
    // And every placed circle is tangent to the two it was placed against.
    for &[a, b, c] in &triangles {
        for (i, j) in [(a, b), (b, c), (c, a)] {
            let dist = libm::hypot(
                positions[i as usize].0 - positions[j as usize].0,
                positions[i as usize].1 - positions[j as usize].1,
            );
            assert!((dist - 2.0).abs() < 1e-9, "{i},{j} at {dist}, not tangent");
        }
    }
}

#[test]
fn a_measured_edge_at_the_guards_own_threshold_still_places_its_third_circle() {
    // `try_place_third` measures `a -> b` rather than using `r_a + r_b` (the reference's
    // own comment, `circle_packing.py:217`), and refuses only at or below 1e-12. Pinned on
    // both sides of that: a hair above it the third circle is placed, at it the placement
    // is refused and the vertex is left for the caller to notice.
    let radii = [1.0, 1.0, 1.0];
    let mut walk = Walk::new(3);
    walk.set(0, (0.0, 0.0));
    walk.set(1, (2e-12, 0.0));
    walk.try_place_third(&radii, 0, 1, 2);
    assert!(walk.is_placed(2), "just above the guard, placed");

    let mut walk = Walk::new(3);
    walk.set(0, (0.0, 0.0));
    walk.set(1, (1e-12, 0.0));
    walk.try_place_third(&radii, 0, 1, 2);
    assert!(!walk.is_placed(2), "at the guard, refused");

    let mut walk = Walk::new(3);
    walk.set(0, (1.5, -2.5));
    walk.set(1, (1.5, -2.5));
    walk.try_place_third(&radii, 0, 1, 2);
    assert!(!walk.is_placed(2), "coincident, refused");
}

#[test]
fn a_third_circle_is_placed_at_the_exact_tangent_corner() {
    // The angle is read off the *measured* `a - b` distance, so rounding drift in the walk
    // cannot break closure: whatever the seeded edge length, the two new edges close.
    for seeded in [2.0_f64, 2.000_000_000_1, 1.999_999_999_9, 2.5] {
        let radii = [1.0, 1.0, 1.0];
        let mut walk = Walk::new(3);
        walk.set(0, (0.0, 0.0));
        walk.set(1, (seeded, 0.0));
        walk.try_place_third(&radii, 0, 1, 2);
        assert!(walk.is_placed(2), "seeded at {seeded}");
        let p = (0.0, 0.0);
        let q = (seeded, 0.0);
        let r = walk.centres[2];
        // Only the two *new* edges close exactly: the seeded edge keeps whatever length
        // the walk gave it, which is what `refine_tangency` is for.
        for (u, v) in [(p, r), (q, r)] {
            let d = libm::hypot(u.0 - v.0, u.1 - v.1);
            assert!(
                (d - 2.0).abs() < 1e-6,
                "seeded at {seeded}: new edge at {d}, not tangent"
            );
        }
    }
}

#[test]
fn a_third_circle_falls_back_to_sixty_degrees_when_the_denominator_underflows() {
    // The other guard: `2 * d_ac * dist_ab` underflowing means the measured edge is
    // vanishingly short next to the target, and the reference answers pi/3. Driven with
    // radii small enough that `d_ac` is itself ~1e-8 while the walk's edge is at 2e-12.
    let radii = [1e-8, 1e-8, 1e-8];
    let mut walk = Walk::new(3);
    walk.set(0, (0.0, 0.0));
    walk.set(1, (2e-12, 0.0));
    walk.try_place_third(&radii, 0, 1, 2);
    assert!(walk.is_placed(2), "placed against a short edge");
    // pi/3 from the origin: the offset's length is `d_ac` at 60 degrees.
    let (x, y) = walk.centres[2];
    let angle = libm::atan2(y, x);
    assert!(
        (angle - core::f64::consts::PI / 3.0).abs() < 1e-9,
        "angle {angle}"
    );
}

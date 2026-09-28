//! The shared finishing steps — `center`, `fit_to_scale` and the `n < 3` special cases —
//! on the boundary each one guards: an empty input, a packing too small to measure, and a
//! packing whose extent is dominated by its radii rather than its centres.

use super::{center, fit_to_scale, pair, single};

fn bits(positions: &[(f64, f64)]) -> Vec<u64> {
    positions
        .iter()
        .flat_map(|&(x, y)| [x.to_bits(), y.to_bits()])
        .collect()
}

#[test]
fn centering_puts_the_mean_at_the_origin() {
    let mut positions = [(-1.0, 2.0), (3.0, -4.0), (5.0, 0.0)];
    center(&mut positions);
    let (mx, my) = (7.0 / 3.0, -2.0 / 3.0);
    let want = [
        (-1.0 - mx, 2.0 - my),
        (3.0 - mx, -4.0 - my),
        (5.0 - mx, 0.0 - my),
    ];
    assert_eq!(bits(&positions), bits(&want));
    let mean_x: f64 = positions.iter().map(|p| p.0).sum::<f64>() / 3.0;
    let mean_y: f64 = positions.iter().map(|p| p.1).sum::<f64>() / 3.0;
    assert!(
        mean_x.abs() < 1e-12 && mean_y.abs() < 1e-12,
        "{mean_x} {mean_y}"
    );
}

#[test]
fn centering_an_empty_packing_is_a_no_op_not_a_division_by_zero() {
    let mut positions: [(f64, f64); 0] = [];
    center(&mut positions);
    assert!(positions.is_empty());
}

#[test]
fn centering_a_single_point_moves_it_to_the_origin() {
    let mut positions = [(7.5, -3.25)];
    center(&mut positions);
    assert_eq!(positions[0], (0.0, 0.0));
}

#[test]
fn fitting_to_the_scale_measures_the_furthest_centre_plus_the_largest_radius() {
    // The reference's own extent (`circle_packing.py:366`): `max |position|` over *both*
    // axes, plus `max(radii)` — not a per-circle `|centre| + r`.
    let mut positions = [(-1.0, 0.0), (2.0, -0.5)];
    let mut radii = [0.25, 0.5];
    fit_to_scale(&mut positions, &mut radii, 5.0);
    // extent 2.0 + 0.5 = 2.5, so the factor is 2.25 / 2.5.
    let factor: f64 = 2.25 / 2.5;
    assert_eq!(positions[0].0.to_bits(), (-factor).to_bits());
    assert_eq!(positions[1].0.to_bits(), (2.0 * factor).to_bits());
    assert_eq!(positions[1].1.to_bits(), (-0.5 * factor).to_bits());
    assert_eq!(radii[0].to_bits(), (0.25 * factor).to_bits());
    assert_eq!(radii[1].to_bits(), (0.5 * factor).to_bits());
}

#[test]
fn fitting_takes_the_extent_from_both_axes() {
    // A packing taller than it is wide: the extent follows the larger, so the y extent is
    // what gets scaled to 0.45 * scale.
    let mut positions = [(0.0, -3.0), (0.0, 1.0)];
    let mut radii = [0.0, 0.0];
    fit_to_scale(&mut positions, &mut radii, 4.0);
    let extent = positions
        .iter()
        .fold(0.0_f64, |m, &(x, y)| m.max(x.abs()).max(y.abs()));
    assert!(
        (extent - 1.8).abs() < 1e-9,
        "y extent {extent} (0.45 * 4 = 1.8)"
    );
}

#[test]
fn fitting_a_packing_too_small_to_measure_leaves_it_alone() {
    // The reference's own `if max_extent > 1e-6` guard: below it, nothing is scaled, so a
    // degenerate packing is not blown up by a division by a near-zero extent.
    let mut positions = [(0.0, 0.0), (0.0, 0.0)];
    let mut radii = [0.0, 0.0];
    fit_to_scale(&mut positions, &mut radii, 5.0);
    assert_eq!(positions, [(0.0, 0.0), (0.0, 0.0)]);
    assert_eq!(radii, [0.0, 0.0]);

    // Both sides of the guard, a decade either side of 1e-6, so a guard that moved by a
    // decade in either direction is caught: 5e-7 stays at full size, 2e-6 is scaled.
    let mut below = [(0.0, 0.0), (5e-7, 0.0)];
    let mut r = [0.0, 0.0];
    fit_to_scale(&mut below, &mut r, 5.0);
    assert_eq!(below[1], (5e-7, 0.0), "5e-7 is a decade under the guard");
    let mut above = [(0.0, 0.0), (2e-6, 0.0)];
    let mut r = [0.0, 0.0];
    fit_to_scale(&mut above, &mut r, 5.0);
    // 2e-6 * (2.25 / 2e-6) is 2.25, the scale factor itself.
    assert!(
        (above[1].0 - 2.25).abs() < 1e-9,
        "2e-6 is a decade over the guard and gets scaled: {}",
        above[1].0
    );
}

#[test]
fn the_scale_is_a_factor_of_zero_point_four_five() {
    let mut positions = [(-1.0, 0.0), (1.0, 0.0)];
    let mut radii = [0.5, 0.5];
    fit_to_scale(&mut positions, &mut radii, 10.0);
    // extent 1.0 + 0.5 = 1.5, so the far centre lands at 0.45 * 10 / 1.5 = 3.0.
    assert!((positions[1].0 - 3.0).abs() < 1e-12, "{}", positions[1].0);
    assert!((radii[0] - 1.5).abs() < 1e-12, "{}", radii[0]);
}

#[test]
fn the_small_packings_are_the_references_own_closed_forms() {
    // `n == 1`: one circle at the origin, radius half the scale
    // (`circle_packing.py:298-299`).
    let one = single(5.0);
    assert_eq!((one.x[0], one.y[0], one.r[0]), (0.0, 0.0, 2.5));
    assert!(!one.approximate, "n < 3 is always exact");
    // `n == 2`: two tangent circles either side of the origin, radius a quarter of the
    // scale (`circle_packing.py:301-303`), touching exactly (2 * 1.25 == 2.5 == scale).
    let two = pair(5.0);
    assert_eq!((two.x[0], two.x[1]), (-1.25, 1.25));
    assert_eq!((two.y[0], two.y[1]), (0.0, 0.0));
    assert_eq!((two.r[0], two.r[1]), (1.25, 1.25));
    assert_eq!(two.x[1] - two.x[0], two.r[0] + two.r[1], "tangent");
    assert!(!two.approximate);
    // The scale is a real factor of it, not a fixed number.
    let big = pair(8.0);
    assert_eq!(big.r[0], 2.0);
    assert_eq!(big.x[1] - big.x[0], big.r[0] + big.r[1], "still tangent");
}
